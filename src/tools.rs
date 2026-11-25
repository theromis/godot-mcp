use crate::bridge::BridgeClient;
use crate::cli_runner::CliRunner;
use crate::config::FeedbackMode;
use crate::error::McpError;
use crate::godot::GameHub;
use crate::journal::EvidenceKind;
use crate::lsp::LspHub;
use crate::paths::{is_valid_identifier, res_display, resolve_in_project};
use crate::protocol::{Tool, ToolAnnotations, ToolContent, ToolResult};
use crate::validation::validate_arguments;
use serde_json::{Value, json};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tokio::fs;
use tokio::io::AsyncWriteExt;

mod addon_update;
mod authoring;
mod code;
mod editor;
mod exit_leaks;
mod export;
mod feedback;
mod file_move;
mod files;
mod game;
mod headless;
mod import_params;
mod input_map;
mod main_loop;
mod pack_run;
mod project;
mod project_gate;
mod reference_kind;
mod resource_props;
mod review;
mod run;
mod scenario;
mod scripts;
mod shaders;
mod world_map;

use headless::ScriptCheck;
use project::EditorChoice;

pub struct ToolManager {
    bridge: BridgeClient,
    cli: CliRunner,
    /// Where the server was started; the project is looked up from here when no
    /// `project_path` is given and no editor is connected.
    working_dir: Option<PathBuf>,
    feedback: feedback::FeedbackState,
    games: GameHub,
    lsp: LspHub,
    /// Editors started by godot_launch_editor, so godot_close_editor can see them exit.
    editors: editor::LaunchedEditors,
    /// Orders the headless Godot processes that work in the same project.
    gates: project_gate::ProjectGates,
    /// The addon the editor of one bridge connection loaded, by the connection's number: read
    /// once per connection, when a call first fails.
    loaded_addon: StdMutex<Option<(u64, addon_update::Loaded)>>,
    /// The folder outside the project that Movie Maker recordings may go to, set by the person.
    movie_dir: Option<PathBuf>,
}

impl ToolManager {
    pub fn new(bridge: BridgeClient, cli: CliRunner) -> Self {
        let godot = cli.binary().to_path_buf();
        Self {
            bridge,
            cli,
            working_dir: std::env::current_dir().ok(),
            feedback: feedback::FeedbackState::new(FeedbackMode::default(), None),
            games: GameHub::new(godot.clone()),
            lsp: LspHub::new(godot),
            editors: editor::LaunchedEditors::default(),
            gates: project_gate::ProjectGates::default(),
            loaded_addon: StdMutex::default(),
            movie_dir: None,
        }
    }

    /// Probes an already-running language server on `port` instead of `GODOT_MCP_LSP_PORT`.
    pub fn with_lsp_port(mut self, port: u16) -> Self {
        self.lsp = LspHub::with_port(self.cli.binary().to_path_buf(), port);
        self
    }

    /// Sets how much of the agent feedback channel is active and where the GitHub client is.
    pub fn with_feedback(mut self, mode: FeedbackMode, gh_path: Option<PathBuf>) -> Self {
        self.feedback = feedback::FeedbackState::new(mode, gh_path);
        self
    }

    /// Lets Movie Maker recordings be written inside `dir` as well as inside the project.
    pub fn with_movie_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.movie_dir = dir.filter(|dir| !dir.as_os_str().is_empty());
        self
    }

    /// Chooses which feedback tools the catalog holds. Call it once at start-up, before the
    /// catalog is first read; a later call has no effect.
    pub fn configure_catalog(mode: FeedbackMode) {
        let _ = CATALOG_MODE.set(mode);
    }

    /// Looks up implicit projects from `dir` instead of the process working directory.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Return the list of all registered tools with their input schemas
    pub fn list_tools() -> Vec<Tool> {
        tool_catalog().to_vec()
    }

    pub fn has_tool(name: &str) -> bool {
        tool_catalog().iter().any(|tool| tool.name == name)
    }

    /// Dispatch tool calls to live bridge or CLI runner
    pub async fn call_tool(self: Arc<Self>, name: &str, args: Value) -> ToolResult {
        let Some(tool) = tool_catalog().iter().find(|tool| tool.name == name) else {
            return ToolResult::error(
                format!("Tool '{name}' is not recognized"),
                &["Check tool name spelling against tools/list"],
            );
        };
        if let Err(problem) = validate_arguments(&tool.input_schema, &args) {
            let result = ToolResult::error(
                format!("Invalid arguments for {name}: {problem}"),
                &["Check the tool's inputSchema returned by tools/list and retry"],
            );
            self.observe(name, &result, EvidenceKind::InvalidArguments);
            return result;
        }

        let result = match name {
            "godot_get_version" => self.handle_get_version().await,
            "godot_get_project_info" => self.handle_bridge_call("get_project_info", args).await,
            "godot_launch_editor" => self.handle_launch_editor(args).await,
            "godot_close_editor" => self.handle_close_editor(args).await,
            "godot_run_project" => self.handle_run_project(args).await,
            "godot_stop_project" => self.handle_stop_project(args).await,
            "godot_get_run_status" => self.handle_run_status().await,
            "godot_get_scene_tree" => self.handle_bridge_call("get_scene_tree", args).await,
            "godot_get_open_scenes" => self.handle_bridge_call("get_open_scenes", args).await,
            "godot_add_node" => self.handle_add_node(args).await,
            "godot_remove_node" => self.handle_bridge_call("remove_node", args).await,
            "godot_get_node_properties" => {
                self.handle_bridge_call("get_node_properties", args).await
            }
            "godot_set_node_properties" => {
                self.handle_bridge_call("set_node_properties", args).await
            }
            "godot_capture_viewport" => self.handle_capture_viewport(args).await,
            "godot_open_scene" => self.handle_bridge_call("open_scene", args).await,
            "godot_save_scene" => self.handle_bridge_call("save_scene", args).await,
            "godot_create_script" => self.handle_create_script(args).await,
            "godot_validate_script" => self.handle_validate_script(args).await,
            "godot_run_script" => self.handle_run_script(args).await,
            "godot_resave_resources" => self.handle_resave_resources(args).await,
            name if authoring::handles(name) => authoring::call(&self, name, args).await,
            name if input_map::handles(name) => input_map::call(&self, name, args).await,
            name if files::handles(name) => files::call(&self, args).await,
            name if file_move::handles(name) => file_move::call(&self, args).await,
            name if scripts::handles(name) => scripts::call(&self, name, args).await,
            name if shaders::handles(name) => shaders::call(&self, name, args).await,
            name if game::handles(name) => game::call(&self, name, args).await,
            name if code::handles(name) => code::call(&self, name, args).await,
            name if export::handles(name) => export::call(&self, name, args).await,
            name if pack_run::handles(name) => pack_run::call(&self, args).await,
            name if addon_update::handles(name) => addon_update::call(&self, args).await,
            name if import_params::handles(name) => import_params::call(&self, name, args).await,
            name if review::handles(name) => review::call(&self, args).await,
            name if resource_props::handles(name) => resource_props::call(&self, args).await,
            name if world_map::handles(name) => world_map::call(&self, args).await,
            feedback::NOTE => self.handle_note_feedback(args).await,
            feedback::DRAFT => self.handle_draft_feedback(args).await,
            feedback::SUBMIT => self.handle_submit_feedback(args).await,
            _ => ToolResult::error(
                format!("Tool '{name}' has no handler"),
                &["This is a server bug; report it with the tool name"],
            ),
        };
        let result = addon_update::explain_unknown_command(&self, name, result).await;
        let result = addon_update::flag_other_build(&self, name, result).await;
        self.observe(name, &result, EvidenceKind::Error);
        result
    }

    async fn handle_get_version(&self) -> ToolResult {
        // Try live editor bridge first
        match self.bridge.send_command("get_version", json!({})).await {
            Ok(result) => {
                return ToolResult::text(format!(
                    "Godot Engine (Live Editor Connected):\n{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                ));
            }
            // An editor that answers with an error (e.g. a token problem) must not be hidden.
            Err(error @ McpError::ToolExecution(_)) => return bridge_error_result(&error),
            Err(_) => {}
        }

        // Fallback to CLI
        match self.cli.get_version().await {
            Ok(ver) => ToolResult::text(format!(
                "Godot Engine (Headless CLI):\nVersion: {ver}\nNote: Editor bridge is offline. Launch Godot Editor and enable Godot MCP Bridge plugin for live manipulation."
            )),
            Err(e) => ToolResult::error(
                format!("Failed to retrieve Godot version: {e}"),
                &[
                    "Verify GODOT_PATH in .env",
                    "Ensure Godot executable is installed and reachable",
                ],
            ),
        }
    }

    async fn handle_bridge_call(&self, command_type: &str, params: Value) -> ToolResult {
        match self.bridge.send_command(command_type, params).await {
            Ok(res) => ToolResult::text(serde_json::to_string_pretty(&res).unwrap_or_default()),
            Err(error) => bridge_error_result(&error),
        }
    }

    /// A node comes from exactly one source: a class (`node_type`) or a saved scene (`scene`).
    async fn handle_add_node(&self, args: Value) -> ToolResult {
        let given = |key: &str| str_arg(&args, key).is_some_and(|value| !value.trim().is_empty());
        match (given("node_type"), given("scene")) {
            (false, false) => ToolResult::error(
                "Pass node_type or scene: the class to create, or the saved scene to instance",
                &[
                    "Pass node_type, such as 'Node2D', or a global script class_name",
                    "Pass scene, the res:// path of a .tscn or .scn file",
                ],
            ),
            (true, true) => ToolResult::error(
                "Pass node_type or scene, not both: an instanced scene's root type comes from the scene",
                &["Drop node_type to instance the scene, or drop scene to create a plain node"],
            ),
            _ => self.handle_bridge_call("add_node", args).await,
        }
    }

    async fn handle_capture_viewport(&self, params: Value) -> ToolResult {
        match self.bridge.send_command("capture_viewport", params).await {
            Ok(res) => {
                let data = res.get("data").and_then(Value::as_str);
                let mime = res.get("mime_type").and_then(Value::as_str);
                match (data, mime) {
                    (Some(data), Some(mime)) => {
                        let note = format!(
                            "Editor viewport capture: {}x{} px ({} view)",
                            res.get("width").and_then(Value::as_u64).unwrap_or(0),
                            res.get("height").and_then(Value::as_u64).unwrap_or(0),
                            res.get("view").and_then(Value::as_str).unwrap_or("2d"),
                        );
                        ToolResult::image_with_text(note, data, mime)
                    }
                    _ => ToolResult::error(
                        "Invalid image response received from the Godot viewport",
                        &["Update the Godot MCP Bridge addon to the same version as the server"],
                    ),
                }
            }
            Err(error) => bridge_error_result(&error),
        }
    }

    async fn handle_launch_editor(&self, args: Value) -> ToolResult {
        let path = match self.host_project(&args) {
            Ok(path) => path,
            Err(result) => return result,
        };

        let launched = self.cli.launch_editor(&path).await.and_then(|child| {
            let pid = crate::cli_runner::process_id(&child, "Godot Editor")?;
            self.editors.add(pid, path.clone(), child);
            Ok(pid)
        });
        match launched {
            Ok(pid) => ToolResult::text(format!(
                "Godot Editor launched successfully for '{}' (PID: {pid}).",
                path.display()
            )),
            Err(e) => ToolResult::error(
                format!("Failed to launch Godot Editor: {e}"),
                &["Verify GODOT_PATH points to a working Godot executable"],
            ),
        }
    }

    async fn handle_create_script(&self, args: Value) -> ToolResult {
        let Some(file_path) = str_arg(&args, "script_path").or_else(|| str_arg(&args, "file_path"))
        else {
            return ToolResult::error(
                "Missing required parameter: script_path (or its older name file_path)",
                &["Pass script_path: a res:// path ending in .gd"],
            );
        };
        if !file_path.ends_with(".gd") {
            return ToolResult::error(
                format!("Script files must have the .gd extension: {file_path}"),
                &["Use a path such as res://scripts/player.gd"],
            );
        }

        let given = str_arg(&args, "content");
        if given.is_some()
            && let Some(header) = ["extends_class", "class_name"]
                .into_iter()
                .find(|key| str_arg(&args, key).is_some())
        {
            return ToolResult::error(
                format!(
                    "Pass either content or {header}, not both: content is written as given, without the standard header"
                ),
                &[&format!(
                    "Put the extends and class_name lines in content and drop {header}"
                )],
            );
        }
        if given.is_some_and(|text| text.len() as u64 > scripts::MAX_BYTES) {
            return ToolResult::error(
                "content is larger than 1 MiB, which the script tools do not handle",
                &["Write a large file with your own file tools"],
            );
        }

        let extends_class = str_arg(&args, "extends_class").unwrap_or("Node");
        if !is_valid_identifier(extends_class) {
            return invalid_identifier("extends_class", extends_class);
        }
        let class_name = str_arg(&args, "class_name");
        if let Some(name) = class_name
            && !is_valid_identifier(name)
        {
            return invalid_identifier("class_name", name);
        }
        let overwrite = args
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let project = match self.resolve_project(&args).await {
            Ok(project) => project,
            Err(result) => return result,
        };
        let target = match resolve_in_project(&project.dir, file_path) {
            Ok(target) => target,
            Err(problem) => {
                return ToolResult::error(
                    problem,
                    &["Use a res:// path or a path relative to the project directory"],
                );
            }
        };

        let res_path = target
            .strip_prefix(&project.dir)
            .ok()
            .map(|relative| format!("res://{}", relative.to_string_lossy().replace('\\', "/")));
        let replacing = overwrite && fs::try_exists(&target).await.unwrap_or(false);
        // A named project may still be the one the editor has open, with the script in a tab;
        // a new script there must reach the editor too, or its class_name stays unknown to it.
        let editor_holds_project = project.from_editor
            || (str_arg(&args, "project_path").is_some()
                && self.editor_may_serve(&project.dir).await);
        let open_tab = match (&res_path, editor_holds_project && replacing) {
            (Some(res_path), true) => match self.script_tab(res_path, false).await {
                Ok(tab) => tab,
                Err(result) => return result,
            },
            _ => None,
        };

        let content = match given {
            Some(text) => text.to_string(),
            None => {
                let mut content = format!("extends {extends_class}\n");
                if let Some(name) = class_name {
                    content.push_str(&format!("class_name {name}\n"));
                }
                content.push_str("\nfunc _ready() -> void:\n\tpass\n");
                content
            }
        };

        if let Some(parent) = target.parent()
            && let Err(err) = fs::create_dir_all(parent).await
        {
            return ToolResult::error(
                format!("Failed to create directory {}: {err}", parent.display()),
                &["Check that the project directory is writable"],
            );
        }
        let mut options = fs::OpenOptions::new();
        options.write(true);
        if overwrite {
            options.create(true).truncate(true);
        } else {
            options.create_new(true);
        }
        let write_result = match options.open(&target).await {
            Ok(mut file) => match file.write_all(content.as_bytes()).await {
                // tokio's File writes on a worker thread and does not flush when dropped, so
                // without this the content can be lost after the tool already reported success.
                Ok(()) => file.flush().await,
                Err(err) => Err(err),
            },
            Err(err) => Err(err),
        };
        if let Err(err) = write_result {
            return if err.kind() == ErrorKind::AlreadyExists {
                ToolResult::error(
                    format!(
                        "File already exists: {}",
                        res_display(&project.dir, &target)
                    ),
                    &["Pass overwrite: true to replace it, or choose another file name"],
                )
            } else {
                ToolResult::error(
                    format!(
                        "Failed to write script file {}: {err}",
                        res_display(&project.dir, &target)
                    ),
                    &["Check that the project directory is writable"],
                )
            };
        }

        let mut reload_note = String::new();
        // Whether an editor took the file in; its scan then registers the class_name.
        let mut editor_scanned = false;
        if editor_holds_project && let Some(res_path) = &res_path {
            // Let the editor pick the file up (and give it a .uid) right away.
            editor_scanned = self
                .bridge
                .send_command("resave_resources", json!({ "paths": [res_path] }))
                .await
                .is_ok();
            if open_tab.is_some() {
                // The refresh above already reloads a clean tab; this call catches an
                // older addon or a tab the refresh did not reach.
                reload_note = match self.script_tab(res_path, true).await {
                    Ok(Some(tab)) if tab.get("stale").and_then(Value::as_bool) == Some(true) => {
                        format!(
                            " Warning: the open script tab still shows the old text; call godot_resave_resources with paths [\"{res_path}\"] to reload it."
                        )
                    }
                    Ok(Some(_)) => {
                        " The open script tab was reloaded with the new text.".to_string()
                    }
                    _ => String::new(),
                };
            }
        }

        // The editor registers a class_name when its scan sees the file; without the editor,
        // nothing would, and every run would fail to find the class.
        if !editor_scanned && let Some(res_path) = &res_path {
            let registration =
                crate::godot::class_cache::register(&project.dir, res_path, &content);
            if let Some(note) = registration.note(res_path) {
                reload_note.push(' ');
                reload_note.push_str(&note);
            }
        }

        let shown = res_path.unwrap_or_else(|| target.display().to_string());
        if given.is_none() {
            return ToolResult::text(format!(
                "Created GDScript file {shown} extending '{extends_class}'.{reload_note}"
            ));
        }
        // Given text can be broken, and a success here would send the caller on to run it.
        let check = json!({ "script_path": shown, "project_path": project.dir });
        match self.check_script(&check).await {
            ScriptCheck::Checked(report) if report.is_error => ToolResult::error(
                format!(
                    "Created GDScript file {shown} from the given content, but it does not validate:\n{}{reload_note}",
                    message_of(&report)
                ),
                &[
                    "Fix it with godot_read_script and godot_edit_script, then run godot_validate_script",
                    "Or call godot_create_script again with overwrite: true and corrected content",
                ],
            ),
            ScriptCheck::Checked(report) => ToolResult::text(format!(
                "Created GDScript file {shown} from the given content.\n{}{reload_note}",
                message_of(&report)
            )),
            ScriptCheck::NoEngine(report) => ToolResult::text(format!(
                "Created GDScript file {shown} from the given content. It was not validated: {}\nRun godot_validate_script once GODOT_PATH points to a working Godot.{reload_note}",
                message_of(&report)
            )),
        }
    }

    /// The editor's script tab for `res_path` when it is open, `None` when it is not or the editor
    /// cannot say (an editor without the command, or none at all). Refuses, as the error, an
    /// overwrite the person could lose typing to: a tab with unsaved changes, or one that cannot
    /// be matched to the file.
    async fn script_tab(&self, res_path: &str, reload: bool) -> Result<Option<Value>, ToolResult> {
        let Ok(tab) = self
            .bridge
            .send_command(
                "script_tab",
                json!({ "script_path": res_path, "reload": reload }),
            )
            .await
        else {
            return Ok(None);
        };
        if reload {
            return Ok(Some(tab));
        }
        if tab.get("dirty").and_then(Value::as_bool) == Some(true) {
            return Err(ToolResult::error(
                format!(
                    "{res_path} has unsaved changes in the Godot script editor, so it was not overwritten"
                ),
                &[
                    "Ask the person to save or discard the changes in the script editor, then call godot_create_script again",
                    "Or change the script with godot_read_script and godot_edit_script once it is saved",
                ],
            ));
        }
        if tab.get("editor_state").and_then(Value::as_str) == Some("unknown") {
            let reason = tab.get("reason").and_then(Value::as_str).unwrap_or("");
            return Err(ToolResult::error(
                format!(
                    "The script editor's tab for {res_path} cannot be matched ({reason}), so it was not overwritten"
                ),
                &[
                    "Close the extra tabs in the script editor, or save the script, then call godot_create_script again",
                ],
            ));
        }
        Ok((tab.get("open").and_then(Value::as_bool) == Some(true)).then_some(tab))
    }
}

/// A result's text without the `Error: ` prefix and the recovery suggestions, to quote it
/// inside another message.
fn message_of(result: &ToolResult) -> String {
    let text: String = result
        .content
        .iter()
        .filter_map(|content| match content {
            ToolContent::Text { text } => Some(text.as_str()),
            ToolContent::Image { .. } => None,
        })
        .collect();
    let text = text.strip_prefix("Error: ").unwrap_or(&text);
    text.split("\n\nActionable recovery suggestions:")
        .next()
        .unwrap_or(text)
        .trim_end()
        .to_string()
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

const MAX_USER_ARGS: usize = 32;

/// The make_editable parameter of the tools that edit a node, which may lie inside an instance.
fn make_editable_param() -> Value {
    json!({
        "type": "boolean",
        "description": "Turn on Editable Children of the instances the node lies in (same undo action); else the edit is refused. Listed in made_editable."
    })
}
const MAX_USER_ARG_CHARS: usize = 256;

/// `Some` when the caller sent `user_args`, including an empty array. `None` when the key is absent.
fn user_args_of(args: &Value) -> Result<Option<Vec<String>>, ToolResult> {
    let Some(value) = args.get("user_args").filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let Some(items) = value.as_array() else {
        return Err(ToolResult::error(
            "user_args must be an array of strings",
            &["Pass user_args: [\"--caustics\"] with mode: \"process\""],
        ));
    };
    if items.len() > MAX_USER_ARGS {
        return Err(ToolResult::error(
            format!("user_args accepts at most {MAX_USER_ARGS} strings"),
            &["Pass only the flags the game itself reads"],
        ));
    }
    let mut user_args = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(ToolResult::error(
                "Every user_args item must be a string",
                &["Pass user_args: [\"--caustics\"]"],
            ));
        };
        if text.chars().count() > MAX_USER_ARG_CHARS {
            return Err(ToolResult::error(
                format!("Each user_args string must be at most {MAX_USER_ARG_CHARS} characters"),
                &["Shorten the argument"],
            ));
        }
        if text.chars().any(char::is_control) {
            return Err(ToolResult::error(
                "user_args cannot contain control characters",
                &["Pass plain text the game reads with OS.get_cmdline_user_args()"],
            ));
        }
        // The runtime reads these two from the same list: a caller's copy would hold or retime a
        // game the server believes is running freely.
        if text.starts_with("--godot-mcp-") {
            return Err(ToolResult::error(
                "user_args cannot start with --godot-mcp-: the server reserves those for itself",
                &[
                    "Use held: true on godot_run_project to start the game held",
                    "Pass only the flags the game itself reads",
                ],
            ));
        }
        user_args.push(text.to_string());
    }
    Ok(Some(user_args))
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn missing_argument(name: &str) -> ToolResult {
    ToolResult::error(
        format!("Missing required parameter: {name}"),
        &["Check the tool's inputSchema returned by tools/list"],
    )
}

fn invalid_identifier(parameter: &str, value: &str) -> ToolResult {
    ToolResult::error(
        format!(
            "Invalid {parameter} {value:?}: use letters, digits and underscores only, not starting with a digit"
        ),
        &["Example: Node2D, CharacterBody3D, Player"],
    )
}

fn with_notice(mut result: ToolResult, notice: &str) -> ToolResult {
    if notice.is_empty() {
        return result;
    }
    if let Some(crate::protocol::ToolContent::Text { text }) = result.content.first_mut() {
        text.push_str(notice);
    }
    result
}

fn list_paths(paths: &[String]) -> String {
    const SHOWN: usize = 32;
    if paths.len() <= SHOWN {
        paths.join(", ")
    } else {
        format!(
            "{}, and {} more",
            paths[..SHOWN].join(", "),
            paths.len() - SHOWN
        )
    }
}

/// What to do about a managed run that has ended. The error names the run.
const RUN_ENDED_HINTS: &[&str] = &[
    "Pass that run_id to godot_run_output to read everything the game printed before it ended",
    "Start the game again with godot_run_project",
];

/// Turns a bridge failure into a tool error whose recovery hints match what actually failed.
fn bridge_error_result(error: &McpError) -> ToolResult {
    match error {
        McpError::BridgeUnavailable(detail) => ToolResult::error(
            format!("The Godot Editor bridge is not reachable ({detail})"),
            &[
                "Launch Godot Editor with your project (godot_launch_editor can do it)",
                "Ensure 'Godot MCP Bridge' plugin is enabled in Project Settings -> Plugins",
                "Start this server in the editor's project directory so both derive the same port, or set the addon's project setting godot_mcp/bridge_port to the port named in the error (the editor's Output panel shows the port it listens on)",
            ],
        ),
        McpError::Bridge(detail) => ToolResult::error(
            detail.clone(),
            &[
                "The editor may be busy (importing, modal dialog) or restarting; retry in a moment",
                "Look for [Godot MCP] messages in the editor's Output panel",
            ],
        ),
        McpError::ToolExecution(message) => {
            ToolResult::error(message.clone(), editor_error_hints(message))
        }
        McpError::RunEnded(report) => ToolResult::error(report.clone(), RUN_ENDED_HINTS),
        McpError::Process(detail) => ToolResult::error(
            format!("Engine process error: {detail}"),
            &["Verify GODOT_PATH points to a working Godot executable"],
        ),
        other => ToolResult::error(
            other.to_string(),
            &["This is unexpected: retry once, and note it with godot_note_feedback if it repeats"],
        ),
    }
}

/// The `code` of a failure the running game reported, which arrives as a JSON object.
fn runtime_code(message: &str) -> Option<String> {
    let value: Value = serde_json::from_str(message).ok()?;
    value.get("code")?.as_str().map(str::to_string)
}

fn editor_error_hints(message: &str) -> &'static [&'static str] {
    if let Some(code) = runtime_code(message) {
        return match code.as_str() {
            "DENIED" => &[
                "The runtime refuses this by design: the script property, private names, and methods that quit, evaluate code, call other methods or free nodes",
                "Use a public property or a gameplay method named in permit",
            ],
            "UNSUPPORTED" => &[
                "If the message says the game has no window, start the game without headless with godot_run_project",
                "Otherwise read the message for what the running game needs",
            ],
            "NOT_FOUND" => &[
                "Look up the node path with godot_game_tree; paths are relative to the current scene",
            ],
            "BAD_PARAMS" => &[
                "Fix the parameter the message names; its shape is in the tool's inputSchema from tools/list",
            ],
            _ => &["Read the message; the running game refused the request"],
        };
    }
    let lower = message.to_lowercase();
    if lower.contains("unauthorized") {
        &[
            "Restart the MCP client so the server re-reads the bridge token",
            "Run the MCP server and the Godot editor as the same OS user, or set the same GODOT_MCP_TOKEN for both",
        ]
    } else if lower.contains("no active scene") {
        &["Open a scene first with godot_open_scene"]
    } else if lower.contains("unsaved changes") {
        &[
            "Save the script in the editor (Ctrl+S) or discard its changes, then call godot_read_script again and repeat the edit",
        ]
    } else if lower.contains("differs from the file") {
        &[
            "Save or reload the script in the editor so that it matches the file, then call godot_read_script again",
        ]
    } else if lower.contains("cannot be matched") {
        &[
            "Close the extra tabs in the script editor (help pages, text files), or save the script, then read it again",
        ]
    } else if lower.contains("already exists") {
        &["Choose another path; an existing file is never replaced"]
    } else if lower.contains("can't open") || lower.contains("failed to save") {
        &[
            "Check that the folder exists or can be created inside the project and that it is writable, then retry",
        ]
    } else if lower.contains("expects a") {
        &[
            "Send a value of the type the setting already has, or change it in the editor's Project Settings",
        ]
    } else if lower.contains("not found")
        || lower.contains("does not exist")
        || lower.contains("not currently open")
    {
        &["Verify the path with godot_get_scene_tree or godot_get_open_scenes"]
    } else {
        &["Correct the arguments according to the message above and retry"]
    }
}

/// Which feedback tools the catalog holds; set once at start-up by `configure_catalog`.
static CATALOG_MODE: OnceLock<FeedbackMode> = OnceLock::new();

fn tool_catalog() -> &'static [Tool] {
    static CATALOG: OnceLock<Vec<Tool>> = OnceLock::new();
    CATALOG.get_or_init(|| build_catalog(CATALOG_MODE.get().copied().unwrap_or_default()))
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Tool {
    // Closed, so a misspelled or invented key is refused instead of silently ignored.
    let mut input_schema = json!({
        "type": "object",
        "properties": properties,
        "additionalProperties": false
    });
    if !required.is_empty() {
        input_schema["required"] = json!(required);
    }
    Tool {
        name: name.into(),
        description: description.into(),
        input_schema,
        annotations: annotations(name),
    }
}

/// Hints for a listed tool. Read-only means it changes nothing in the project,
/// the editor, a running game or files on disk; scratch state kept by this
/// server (screenshot baselines) does not count. Destructive means it may
/// overwrite or remove something there, or run code that can. Idempotent means
/// repeating the call with the same arguments changes nothing more. Only
/// godot_submit_feedback reaches outside this machine. A tool missing here is
/// listed without hints, which hosts read as the most cautious defaults.
fn annotations(name: &str) -> Option<ToolAnnotations> {
    let [read_only, destructive, idempotent] = match name {
        "godot_get_version"
        | "godot_get_project_info"
        | "godot_get_run_status"
        | "godot_get_scene_tree"
        | "godot_get_open_scenes"
        | "godot_get_node_properties"
        | "godot_validate_script"
        | "godot_list_signals"
        | "godot_get_project_settings"
        | "godot_get_input_map"
        | "godot_read_script"
        | "godot_read_shader"
        | "godot_validate_shader"
        | "godot_run_output"
        | "godot_game_tree"
        | "godot_game_get"
        | "godot_game_signals"
        | "godot_game_observe"
        | "godot_game_snapshot"
        | "godot_game_state"
        | "godot_game_screenshot"
        | "godot_game_probe"
        | "godot_debug_state"
        | "godot_code_diagnostics"
        | "godot_code_complete"
        | "godot_code_symbols"
        | "godot_code_definition"
        | "godot_code_references"
        | "godot_list_export_presets"
        | "godot_get_import_params" => [true, false, true],
        "godot_capture_viewport"
        | "godot_review_assets"
        | "godot_world_map"
        | "godot_resave_resources"
        | "godot_create_project"
        | "godot_create_scene"
        | "godot_connect_signal"
        | "godot_create_resource"
        | "godot_game_input_release"
        | "godot_game_wait"
        | "godot_debug_breakpoints" => [false, false, true],
        "godot_launch_editor"
        | "godot_run_project"
        | "godot_run_pack"
        | "godot_add_node"
        | "godot_game_input"
        | "godot_game_physics"
        | "godot_game_scenario"
        | "godot_game_record"
        | "godot_debug_control"
        | "godot_note_feedback"
        | "godot_draft_feedback"
        | "godot_submit_feedback" => [false, false, false],
        "godot_stop_project"
        | "godot_remove_node"
        | "godot_set_node_properties"
        | "godot_set_resource_properties"
        | "godot_open_scene"
        | "godot_save_scene"
        | "godot_create_script"
        | "godot_create_shader"
        | "godot_attach_script"
        | "godot_set_project_setting"
        | "godot_edit_input_action"
        | "godot_set_export_preset"
        | "godot_set_import_params"
        | "godot_delete_file"
        | "godot_update_addon"
        | "godot_game_set" => [false, true, true],
        "godot_run_script"
        | "godot_edit_script"
        | "godot_edit_shader"
        | "godot_game_call"
        | "godot_game_command"
        | "godot_game_run_script"
        | "godot_close_editor"
        | "godot_move_file"
        | "godot_export_project" => [false, true, false],
        _ => return None,
    };
    Some(ToolAnnotations {
        read_only_hint: read_only,
        destructive_hint: destructive,
        idempotent_hint: idempotent,
        open_world_hint: name == "godot_submit_feedback",
    })
}

fn build_catalog(feedback_mode: FeedbackMode) -> Vec<Tool> {
    let mut tools = vec![
        tool(
            "godot_get_version",
            "Godot Engine version and build information.",
            json!({}),
            &[],
        ),
        tool(
            "godot_get_project_info",
            "Name, main scene, enabled features and directory of the project open in the editor. Requires the editor bridge.",
            json!({}),
            &[],
        ),
        tool(
            "godot_launch_editor",
            "Launches the Godot Editor for a project. Without project_path, opens the project the server was started in (its working directory or a parent, or the only subfolder with project.godot).",
            json!({
                "project_path": {
                    "type": "string",
                    "description": "Absolute project directory (holding project.godot). Optional inside the project."
                }
            }),
            &[],
        ),
        tool(
            "godot_close_editor",
            "Closes a project's editor (project_path, else the server's project, else the connected editor's) and waits for its process to exit. A connected editor quits without saving: with unsaved scenes or scripts it stays open and the answer lists them, unless discard is true. An editor that has another project open is never closed. Without a connected editor, only an editor godot_launch_editor started can be stopped, by its PID, and only with discard true.",
            json!({
                "project_path": {
                    "type": "string",
                    "description": "Project directory. Optional inside the project."
                },
                "discard": {
                    "type": "boolean",
                    "description": "Close despite unsaved scenes or scripts, losing them. Required to stop a launched editor not connected to the bridge. Default false."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 120,
                    "description": "Seconds to wait for the process to exit. Default 30."
                }
            }),
            &[],
        ),
        tool(
            "godot_run_project",
            "Runs the main or a given scene: in editor Play when the bridge is connected (mode auto), else or with mode process as a managed debugger-attached process (godot_stop_project stops it). held, user_args and movie need mode process. held starts it held at the first physics tick at a fixed frame rate, for exact godot_game_physics steps. movie records with Movie Maker, not in real time (each frame advances game time 1/fps); not with held or headless. Quality, mix rate and vsync are editor/movie_writer/* project settings. The file is finished when the game quits (quit_after_frames, itself, godot_stop_project); godot_get_run_status reports movie. Stale imports are reimported first when the editor has this project, else warned. A process run waits for an import or export running in the project.",
            json!({
                "project_path": {
                    "type": "string",
                    "description": "Project directory, used only without an editor. Default: the server's project."
                },
                "scene": {
                    "type": "string",
                    "description": "res:// scene. Default: the project's main scene."
                },
                "breakpoints": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "res://script.gd:LINE breakpoints (process runs)."
                },
                "headless": {
                    "type": "boolean",
                    "description": "Process runs: start without a window (no screenshots)."
                },
                "break_on_errors": {
                    "type": "boolean",
                    "description": "Process runs: a script error pauses the game. Default false (logged, game keeps running)."
                },
                "held": {
                    "type": "boolean",
                    "description": "Needs mode process. Start held at the first physics tick, for exact steps."
                },
                "mode": {
                    "type": "string",
                    "enum": ["auto", "process"],
                    "description": "auto (default): editor Play when the bridge is connected, else a process. process: always a managed debugger-attached process."
                },
                "user_args": {
                    "type": "array",
                    "items": {"type": "string"},
                    "maxItems": 32,
                    "description": "Mode process only (refused otherwise). Up to 32 strings of at most 256 characters, passed after --."
                },
                "movie": {
                    "type": "object",
                    "description": "Mode process only; not with held or headless. Record with Godot's Movie Maker.",
                    "properties": {
                        "path": {"type": "string", "description": ".avi (MJPEG with audio) or .png (name########.png, audio in name.wav). res://, project-relative, or absolute inside the project or GODOT_MCP_MOVIE_DIR. Its folder is created (with a .gdignore inside the project)."},
                        "fps": {"type": "integer", "minimum": 1, "maximum": 1000, "description": "Recording and game frame rate (--fixed-fps). Default 60."},
                        "quit_after_frames": {"type": "integer", "minimum": 1, "maximum": 10000000, "description": "Quit after this many frames (seconds times fps), finishing the file. Omit to record until the game quits or is stopped."},
                        "overwrite": {"type": "boolean", "description": "Replace an earlier recording at the path. Default false."}
                    },
                    "required": ["path"]
                }
            }),
            &[],
        ),
        tool(
            "godot_stop_project",
            "Without run_id, stops the editor's Play mode and every running managed process run, and lists what it stopped (or that nothing ran). run_id stops one process run.",
            json!({
                "run_id": {"type": "string", "description": "Managed process run id. Omit to stop everything."}
            }),
            &[],
        ),
        tool(
            "godot_get_run_status",
            "Whether the editor plays a scene, and the managed process runs with their user_args and, for a Movie Maker run, movie (path, size_bytes, frames, duration_seconds, finished).",
            json!({}),
            &[],
        ),
        tool(
            "godot_get_scene_tree",
            "Node hierarchy of an open scene; paths are relative to the root ('.'). Does not reload: stale is true when the file on disk differs. An instance node has instance (res:// path) and editable_children; inside a non-editable instance the editing tools refuse changes unless make_editable is passed.",
            json!({
                "scene_path": {
                    "type": "string",
                    "description": "res:// path of an open tab (see godot_get_open_scenes). Default: the active tab."
                }
            }),
            &[],
        ),
        tool(
            "godot_get_open_scenes",
            "res:// paths of the open scene tabs, and the active one.",
            json!({}),
            &[],
        ),
        tool(
            "godot_add_node",
            "Adds a node to the active scene (undoable). Pass exactly one of node_type (an instantiable Node class or global class_name; a non-@tool script class gets its native base with the script attached) or scene (a saved .tscn/.scn added as an instance that keeps its uid). A scene that is missing, not a PackedScene, or contains the edited scene is refused. properties are converted and checked as in godot_set_node_properties; a bad value, or one Godot does not keep, rejects the call. applied lists the values as read back.",
            json!({
                "parent_node_path": {
                    "type": "string",
                    "description": "Parent path relative to the scene root ('.' for the root)."
                },
                "node_type": {
                    "type": "string",
                    "description": "Class to instantiate, e.g. Sprite2D. Not with scene."
                },
                "scene": {
                    "type": "string",
                    "description": "res:// scene to add as an instance. Not with node_type."
                },
                "node_name": {
                    "type": "string",
                    "description": "Name; Godot may replace invalid characters (the result has the final name). An instance defaults to its root's name."
                },
                "properties": {
                    "type": "object",
                    "description": "Initial values, e.g. {\"position\": [10, 0, 5]}. A node reference path starts at the new node, so '../Sibling' names a child of the parent."
                },
                "make_editable": make_editable_param()
            }),
            &[],
        ),
        tool(
            "godot_remove_node",
            "Removes a node from the active scene (undoable). Refuses the scene root and a node an instanced scene brings (Godot cannot save that deletion): hide it with godot_set_node_properties or remove it in its own scene.",
            json!({
                "node_path": {
                    "type": "string",
                    "description": "Node path relative to the scene root."
                }
            }),
            &["node_path"],
        ),
        tool(
            "godot_get_node_properties",
            "Reads a node's editor-visible properties as {type, value} in Godot's text format. properties reads only those names; ':' steps through a resource (material_override:shader_parameter/glow_amount). A name that cannot be read goes to property_errors; the rest are returned.",
            json!({
                "node_path": {
                    "type": "string",
                    "description": "Relative to the scene root ('.' or empty for the root)."
                },
                "properties": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Names to read instead of all; ':' steps through resources."
                }
            }),
            &["node_path"],
        ),
        tool(
            "godot_set_node_properties",
            "Sets node properties in the active scene in one undo action, all or nothing. Values convert to the property's type: arrays or {x, y} for vectors and colors, '#rrggbb' or names for colors, res:// paths for resources, Godot literals ('Vector2(1, 2)'). A node reference (@export var target: Node3D) takes a path relative to the edited node to a node of its type ('../King', '..' for the parent, null clears). 'metadata/<name>' (name an identifier) keeps an existing entry's type, else takes the JSON value's type; not null. ':' steps through a resource built into this scene (material_override:shader_parameter/glow_amount); a shared one (a .tres, another scene's) is refused: use godot_set_resource_properties. script and private segments after ':' are refused. An unknown property, an unconvertible value or one that reads back differently (a clamping setter, an audio bus missing from the layout) rejects the call. applied lists the values as read back. A node in a non-editable instance is refused unless make_editable.",
            json!({
                "node_path": {
                    "type": "string",
                    "description": "Relative to the scene root ('.' for the root)."
                },
                "properties": {
                    "type": "object",
                    "description": "Property name to value; ':' steps through resources."
                },
                "make_editable": make_editable_param()
            }),
            &["node_path", "properties"],
        ),
        tool(
            "godot_capture_viewport",
            "PNG screenshot of the editor's 2D or 3D viewport (scene content, no gizmos or grid). Switches the editor's main screen; needs a windowed editor with a scene open.",
            json!({
                "view": {
                    "type": "string",
                    "enum": ["auto", "2d", "3d"],
                    "description": "auto (default): 3D for Node3D scenes, else 2D."
                },
                "max_dimension": {
                    "type": "integer",
                    "description": "Longest side in pixels after downscaling. Default 1568."
                }
            }),
            &[],
        ),
        tool(
            "godot_open_scene",
            "Opens a scene by res:// path (or absolute inside the project) as the active tab (active_scene). Reloading an open scene drops its undo history. load_errors lists what Godot reported while loading; unsaved (Godot 4.7+) says whether the load left unsaved changes.",
            json!({
                "scene_path": {
                    "type": "string",
                    "description": "res:// path, e.g. res://scenes/level1.tscn."
                },
                "reload": {
                    "type": "boolean",
                    "description": "Reload an open scene from disk, dropping its undo history. Automatic when the file changed and the editor has no unsaved changes."
                },
                "discard": {
                    "type": "boolean",
                    "description": "Reload even over unsaved editor changes, discarding them and the undo history."
                }
            }),
            &["scene_path"],
        ),
        tool(
            "godot_save_scene",
            "Saves the active scene and returns scene_path. Open script tabs with unsaved typing are saved too (scripts_saved); clean scripts are not rewritten.",
            json!({}),
            &[],
        ),
        tool(
            "godot_create_script",
            "Creates a .gd file from content, or a header (extends_class, class_name, an empty _ready). Validated after writing: an invalid script is an error but stays for godot_edit_script. Refuses an existing file unless overwrite is true; an overwrite is refused while the editor tab has unsaved changes, else the tab reloads. Without the editor, a class_name is added to .godot/global_script_class_cache.cfg so runs know it at once.",
            json!({
                "script_path": {
                    "type": "string",
                    "description": "res:// or project-relative .gd path."
                },
                "file_path": {
                    "type": "string",
                    "description": "Older name of script_path."
                },
                "content": {
                    "type": "string",
                    "description": "Full script text (up to 1 MiB). Not with extends_class or class_name."
                },
                "extends_class": {
                    "type": "string",
                    "description": "Header base class. Default Node. Not with content."
                },
                "class_name": {
                    "type": "string",
                    "description": "Header global class_name. Not with content."
                },
                "project_path": {
                    "type": "string",
                    "description": "Default: editor's, else server's project."
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Replace an existing file. Default false."
                }
            }),
            &[],
        ),
        tool(
            "godot_validate_script",
            "Checks a GDScript file. With a running language server (the editor's, or one godot_code_diagnostics started) returns errors and warnings (file:line:col, 1-based; code; message); a warning does not fail the check, an error does. A class it calls unknown that the project declares is registered through the editor and rechecked. Without a server, runs godot --headless --check-only: parse errors only, no warnings. A script using a class missing from .godot/global_script_class_cache.cfg fails the check (runs cannot find it until godot_resave_resources or the editor registers it).",
            json!({
                "script_path": {
                    "type": "string",
                    "description": ".gd path (res:// needs a project)."
                },
                "project_path": {
                    "type": "string",
                    "description": "For class resolution. Default: editor's, else server's project."
                }
            }),
            &["script_path"],
        ),
        tool(
            "godot_run_script",
            "Runs a SceneTree or MainLoop script in headless Godot (-s <script> -- user_args); another base or a path outside the project is refused. Returns the exit code, stdout and stderr (last 64 KiB each; the whole stream is in stdout_log/stderr_log, overwritten by the next run) and script_errors (file, line). An unloadable script is an error with load_error, even if Godot exits 0. A SCRIPT ERROR that leaves the main loop idle ends the run. Repeated exit leak lines are shown once with xN (exit_leaks, hint). On timeout the process tree is killed and the output so far returned, so print as you go (stdout_buffered true: unflushed print() was lost; use printerr()). Progress notifications carry the newest line. Stale imports are reimported first when the editor has this project; else the script runs on the old import data and import_warning names godot_resave_resources. Dropped uid:// references are restored (resource_repairs; binary_uid_error, resource_damage: what is still missing), uids recorded in the uid cache (uid_cache_stale if not), and an open editor reloads what was written (editor_refresh). Lists stop at 20 (<list>_total). Parallel calls each get their own Godot and must write different files (overlapped: repairs waited).",
            json!({
                "script": {
                    "type": "string",
                    "description": "res:// or project path of the .gd script."
                },
                "user_args": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Arguments after --, read by OS.get_cmdline_user_args(). At most 32."
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Seconds before the process tree is killed. Default 120, max 3600; the client's timeout may end the call sooner.",
                    "minimum": 1,
                    "maximum": 3600
                },
                "fixed_fps": {
                    "type": "integer",
                    "description": "Run unthrottled at this many frames per game second (--fixed-fps), e.g. 60. Wall-clock time (Time.get_ticks_msec), threads and background loads no longer match game time: wait on their status, not frames. Omit for real time.",
                    "minimum": 1,
                    "maximum": 1000
                },
                "project_path": {
                    "type": "string",
                    "description": "Default: editor's, else server's project."
                }
            }),
            &["script"],
        ),
        tool(
            "godot_resave_resources",
            "Refreshes files changed outside the editor. With the editor: known importable files reimport synchronously, others are re-scanned; clean script tabs reload (reloaded_tabs), ones with unsaved changes are kept (unsaved_tabs); loaded shaders, resources and tabless scripts reload in place (reloaded_resources). A never-scanned path without a resulting .import is listed under not_scanned. Open scenes are not reloaded: use godot_open_scene reload. Without paths, rescans the whole project in the background. Without the editor, runs `godot --headless --path <project> --import` and lists imported sources. A requested path whose .godot/imported data still differs from its source is listed in still_stale with a reason and makes the call an error (force true reimports it headless). The headless import waits for godot_run_script calls in the project and runs alone.",
            json!({
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "res:// paths to refresh. Omit or empty for a full rescan."
                },
                "wait": {
                    "type": "boolean",
                    "description": "Full editor scan only: return when scanning and importing finish (up to 60s), with imported (count) and timed_out. Ignored with paths. Default false. The headless import always waits."
                },
                "force": {
                    "type": "boolean",
                    "description": "With paths, without the editor: reimport even when Godot's cache says current; listed in forced. Default false."
                },
                "project_path": {
                    "type": "string",
                    "description": "Project for the headless import (when the editor is closed or has another project). Default: the server's project."
                }
            }),
            &[],
        ),
    ];
    tools.extend(authoring::catalog());
    tools.extend(resource_props::catalog());
    tools.extend(input_map::catalog());
    tools.extend(files::catalog());
    tools.extend(file_move::catalog());
    tools.extend(scripts::catalog());
    tools.extend(shaders::catalog());
    tools.extend(addon_update::catalog());
    tools.extend(game::catalog());
    tools.extend(code::catalog());
    tools.extend(export::catalog());
    tools.extend(pack_run::catalog());
    tools.extend(import_params::catalog());
    tools.extend(review::catalog());
    tools.extend(world_map::catalog());
    tools.extend(feedback::catalog(feedback_mode));
    tools
}
