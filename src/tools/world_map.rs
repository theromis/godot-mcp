//! A top-down map of a world split into many scene files (streamed zones, say), without running
//! the game: the scenes are placed at their saved transforms in a temporary Godot process that
//! renders (a small window, since a headless Godot draws nothing), and the answer is one PNG with
//! each scene's bounds and file name, and a summary of every scene that shows at once which ones
//! hold no collision or no visible geometry.
//!
//! The process runs a fixed script (`world_map.gd`) from a scratch directory outside the project
//! and reads the checked request from a JSON file there, so no argument reaches the command line
//! or generated source, and nothing is saved into the project. It loads one scene at a time to
//! measure it and renders the map in tiles, so memory is bounded by the scenes overlapping one
//! tile rather than by the whole world.

use super::ToolManager;
use crate::paths::{res_display, resolve_in_project, skipped_by_godot};
use crate::protocol::{Tool, ToolContent, ToolResult};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

pub const NAME: &str = "godot_world_map";

const SCRIPT: &str = include_str!("world_map.gd");
const MAX_SCENES: usize = 256;
const MAX_SIDE: f64 = 4096.0;
const DEFAULT_SIDE: f64 = 2048.0;
const DEFAULT_TIMEOUT: u64 = 300;
const MAX_TIMEOUT: u64 = 3600;
const SCENE_EXTENSIONS: &[&str] = &["tscn", "scn"];

pub fn handles(name: &str) -> bool {
    name == NAME
}

pub fn catalog() -> Vec<Tool> {
    vec![super::tool(
        NAME,
        "Draws a map of a world split into many scene files (streamed zones, tiles of a level), without running the game, and summarises every scene, so a scene with no ground or no geometry shows at once. The scenes come from dir and glob (file names in that directory, recursive optional) and/or a list of scenes; each is instanced at the transform saved in its file. A temporary Godot process (a small window, since a headless Godot does not render; audio muted) loads the scenes one at a time to measure them, then renders the map top-down with an orthographic camera in tiles, instancing only the scenes that touch the tile, so memory stays bounded by one tile's scenes; the scenes' WorldEnvironment nodes are dropped, their cameras and 2D layers are turned off, nothing of theirs processes (their _ready still runs), and the light is a neutral sun without shadows. view \"collision\" draws the collision shapes as solid stand-ins instead of the visible geometry, which shows holes in the ground. labels draws each scene's bounds and file name faintly over the map, in red with \"(no collision)\" or \"(no geometry)\" where one is missing. Returns the PNG (top is -Z; a pixel (px, py) is the world point x = rect[0] + px / pixels_per_metre, z = rect[1] + py / pixels_per_metre) and JSON: image (size, rect, pixels_per_metre, tiles, the most scenes loaded at once), totals (scenes, no_collision, no_geometry, failed), problems (one line per scene with something missing or failing to load) and per scene its root (name, type, saved position and rotation), top_level child names, bounds (world AABB of its visible geometry, or null), collision_bounds (world AABB of its enabled collision shapes, or null), mesh_instances, multimesh_instances, collision_objects, collision_shapes, collision_layers, instanced_scenes (nested scene instances), has_geometry, has_collision and load_ms. Scenes must be imported: stale imports are named in a warning. Needs GODOT_PATH and a display. The engine may update caches under the project's .godot folder, as any run does.",
        json!({
            "dir": {"type": "string", "description": "res:// directory to take scenes from, such as res://scenes/regions/old_pond."},
            "glob": {"type": "string", "description": "File name pattern in dir, with * and ?, such as zone_*.tscn. Default *.tscn. Matches names only, not directories."},
            "recursive": {"type": "boolean", "description": "Also look in dir's subdirectories (those Godot does not skip). Default false."},
            "scenes": {
                "type": "array",
                "items": {"type": "string"},
                "description": "res:// paths of .tscn or .scn files, added to those found in dir. 1 to 256 scenes in all."
            },
            "rect": {
                "type": "array",
                "items": {"type": "number"},
                "description": "[min_x, min_z, max_x, max_z] in metres: the area to draw. Default: the bounds of all scenes and a small margin."
            },
            "pixels_per_metre": {"type": "number", "minimum": 0.01, "maximum": 64, "description": "Map scale. Default: the longer side is 2048 pixels. The map is at most 4096 pixels on a side."},
            "view": {"type": "string", "enum": ["geometry", "collision"], "description": "geometry (default) draws what the scenes show; collision draws their collision shapes instead."},
            "labels": {"type": "boolean", "description": "Draw each scene's bounds and file name over the map. Default true."},
            "tile_metres": {"type": "number", "minimum": 1, "maximum": 100000, "description": "Side of a render tile in metres. Default: chosen so a tile holds at most 9 scenes and at most 1024 pixels."},
            "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": MAX_TIMEOUT, "description": "Default 300."},
            "project_path": {"type": "string"}
        }),
        &[],
    )]
}

pub async fn call(tools: &ToolManager, args: Value) -> ToolResult {
    let project = match tools.resolve_project(&args).await {
        Ok(project) => project,
        Err(result) => return result,
    };
    let request = match map_request(&project.dir, &args) {
        Ok(request) => request,
        Err(problem) => {
            return ToolResult::error(
                format!("Invalid world map request: {problem}"),
                &[
                    "Check the arguments against the godot_world_map inputSchema and retry",
                    "List the directory with godot_list_files to see which scenes it holds",
                ],
            );
        }
    };
    let timeout_seconds = args
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT)
        .clamp(1, MAX_TIMEOUT);
    let imports = tools.check_imports(&project.dir).await;
    let outcome = match tools
        .cli
        .world_map(&project.dir, SCRIPT, request, timeout_seconds)
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            return ToolResult::error(
                format!("Failed to run the world map: {error}"),
                &["Verify GODOT_PATH points to a working Godot executable"],
            );
        }
    };
    let run = outcome.run;
    let answer = outcome
        .answer
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let Some(mut answer) = answer else {
        let detail = if run.timed_out {
            format!("Godot did not finish the map within {timeout_seconds}s")
        } else {
            format!(
                "Godot gave no answer (exit {})",
                run.exit_code
                    .map_or_else(|| "unknown".to_string(), |code| code.to_string())
            )
        };
        return ToolResult::error(
            format!("{detail}. Its output ends with:\n{}", output_tail(&run)),
            &[
                "Raise timeout_seconds, or map fewer scenes at once (a narrower glob or rect)",
                "Godot must be able to open a window: run the server in a desktop session (a headless Godot does not render)",
                "A scene's own script may fail on load: open the scene named in the output in the editor",
            ],
        );
    };
    if let Some(problem) = answer.get("error").and_then(Value::as_str) {
        return ToolResult::error(
            format!("The world map failed: {problem}"),
            &[
                "Check that the scenes are imported (godot_resave_resources) and load in the editor",
                "Pass rect and pixels_per_metre explicitly when the scenes' bounds are not what you expect",
            ],
        );
    }
    let Some(png) = outcome.image.filter(|bytes| bytes.starts_with(b"\x89PNG")) else {
        return ToolResult::error(
            "The world map ran but saved no picture",
            &["Retry; if it repeats, run the project once to check that it renders"],
        );
    };
    if let Some(map) = answer.as_object_mut() {
        add_run_warnings(map, &run, imports.as_ref());
    }
    ToolResult {
        content: vec![
            ToolContent::Text {
                text: serde_json::to_string_pretty(&answer).unwrap_or_default(),
            },
            ToolContent::Image {
                data: base64_encode(&png),
                mime_type: "image/png".into(),
            },
        ],
        is_error: false,
    }
}

/// Script errors printed while the scenes loaded (their own scripts), and stale imports.
fn add_run_warnings(
    answer: &mut Map<String, Value>,
    run: &crate::cli_runner::ScriptRun,
    imports: Option<&super::run::StaleImports>,
) {
    let mut warnings: Vec<Value> = answer
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for error in run.script_errors.iter().take(10) {
        warnings.push(json!(format!(
            "script error in {}:{}: {}",
            error.file, error.line, error.message
        )));
    }
    if let Some(stale) = imports {
        warnings.push(json!(stale.sentence("map")));
    }
    answer.insert("warnings".into(), Value::Array(warnings));
}

fn output_tail(run: &crate::cli_runner::ScriptRun) -> String {
    let output = format!("{}\n{}", run.stdout.trim(), run.stderr.trim());
    let output = output.trim();
    let start = output
        .char_indices()
        .rev()
        .nth(1999)
        .map_or(0, |(index, _)| index);
    output[start..].to_string()
}

/// Checks the arguments and turns them into the request the map script reads: the scenes as
/// sorted, distinct res:// paths of existing scene files of the project, and checked numbers.
pub(crate) fn map_request(project: &Path, args: &Value) -> Result<Value, String> {
    let mut scenes: Vec<String> = Vec::new();
    if let Some(dir) = args.get("dir") {
        let dir = dir.as_str().ok_or("dir must be a res:// directory")?;
        let glob = match args.get("glob") {
            None => "*.tscn",
            Some(value) => value.as_str().ok_or("glob must be a string")?,
        };
        if glob.is_empty() || glob.contains('/') || glob.contains('\\') {
            return Err(format!(
                "glob matches file names in dir, so it must be a non-empty name pattern without / : {glob}"
            ));
        }
        let recursive = match args.get("recursive") {
            None => false,
            Some(value) => value.as_bool().ok_or("recursive must be true or false")?,
        };
        let root = resolve_in_project(project, dir).map_err(|problem| format!("dir: {problem}"))?;
        if !root.is_dir() {
            return Err(format!(
                "dir: no such directory in the project: {}",
                res_display(project, &root)
            ));
        }
        let found = scene_files(&root, glob, recursive)?;
        if found.is_empty() {
            return Err(format!(
                "no .tscn or .scn file in {} matches {glob}{}",
                res_display(project, &root),
                if recursive { " (recursively)" } else { "" }
            ));
        }
        scenes.extend(found.iter().map(|file| res_display(project, file)));
    } else if args.get("glob").is_some() || args.get("recursive").is_some() {
        return Err("glob and recursive need dir".into());
    }
    if let Some(list) = args.get("scenes") {
        let list = list
            .as_array()
            .ok_or("scenes must be a list of res:// scene paths")?;
        for (index, scene) in list.iter().enumerate() {
            let field = format!("scenes[{index}]");
            let scene = scene
                .as_str()
                .ok_or(format!("{field} must be a res:// scene path"))?;
            scenes.push(scene_file(project, scene, &field)?);
        }
    }
    scenes.sort();
    scenes.dedup();
    if scenes.is_empty() {
        return Err("give dir (with glob) or scenes".into());
    }
    if scenes.len() > MAX_SCENES {
        return Err(format!(
            "{} scenes match, more than {MAX_SCENES}; narrow glob or map the world in parts",
            scenes.len()
        ));
    }

    let mut request = Map::new();
    request.insert("scenes".into(), json!(scenes));
    let rect = match args.get("rect") {
        None => None,
        Some(value) => {
            let parts = value
                .as_array()
                .filter(|parts| parts.len() == 4)
                .ok_or("rect must be [min_x, min_z, max_x, max_z]")?
                .iter()
                .map(|part| part.as_f64().filter(|number| number.is_finite()))
                .collect::<Option<Vec<f64>>>()
                .ok_or("rect must hold four numbers")?;
            if parts[2] <= parts[0] || parts[3] <= parts[1] {
                return Err("rect must be [min_x, min_z, max_x, max_z] with max above min".into());
            }
            request.insert("rect".into(), json!(parts));
            Some(parts)
        }
    };
    if let Some(value) = args.get("pixels_per_metre") {
        let ppm = value
            .as_f64()
            .filter(|ppm| (0.01..=64.0).contains(ppm))
            .ok_or("pixels_per_metre must be a number from 0.01 to 64")?;
        if let Some(rect) = &rect {
            let longest = (rect[2] - rect[0]).max(rect[3] - rect[1]);
            if (longest * ppm).ceil() > MAX_SIDE {
                return Err(format!(
                    "the map would be {} pixels on its longer side, more than {MAX_SIDE}; use pixels_per_metre {:.3} or less for this rect",
                    (longest * ppm).ceil(),
                    MAX_SIDE / longest
                ));
            }
        }
        request.insert("pixels_per_metre".into(), json!(ppm));
    }
    request.insert("default_side".into(), json!(DEFAULT_SIDE));
    let view = match args.get("view") {
        None => "geometry",
        Some(value) => match value.as_str() {
            Some(view @ ("geometry" | "collision")) => view,
            _ => return Err("view must be \"geometry\" or \"collision\"".into()),
        },
    };
    request.insert("view".into(), json!(view));
    let labels = match args.get("labels") {
        None => true,
        Some(value) => value.as_bool().ok_or("labels must be true or false")?,
    };
    request.insert("labels".into(), json!(labels));
    if let Some(value) = args.get("tile_metres") {
        let tile = value
            .as_f64()
            .filter(|tile| (1.0..=100_000.0).contains(tile))
            .ok_or("tile_metres must be a number from 1 to 100000")?;
        request.insert("tile_metres".into(), json!(tile));
    }
    Ok(Value::Object(request))
}

/// The res:// path of an existing scene file of the project.
fn scene_file(project: &Path, requested: &str, field: &str) -> Result<String, String> {
    let file =
        resolve_in_project(project, requested).map_err(|problem| format!("{field}: {problem}"))?;
    if !is_scene(&file) {
        return Err(format!("{field} must be a .tscn or .scn file: {requested}"));
    }
    if !file.is_file() {
        return Err(format!(
            "{field}: no such file in the project: {}",
            res_display(project, &file)
        ));
    }
    Ok(res_display(project, &file))
}

fn is_scene(file: &Path) -> bool {
    file.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            SCENE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

/// Scene files under root whose names match glob, skipping what Godot leaves out of the project.
fn scene_files(root: &Path, glob: &str, recursive: bool) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|error| format!("dir: cannot list {}: {error}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_dir = path.is_dir();
            if skipped_by_godot(&path, is_dir) {
                continue;
            }
            if is_dir {
                if recursive {
                    pending.push(path);
                }
                continue;
            }
            let matches = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| wildcard_match(glob, name));
            if matches && is_scene(&path) {
                found.push(path);
            }
        }
    }
    Ok(found)
}

/// Whether name matches pattern, where * stands for any run of characters and ? for one.
fn wildcard_match(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut p, mut n) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, n));
            p += 1;
        } else if let Some((star_p, star_n)) = star {
            p = star_p + 1;
            n = star_n + 1;
            star = Some((star_p, star_n + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = (u32::from(chunk[0]) << 16)
            | (u32::from(chunk.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(chunk.get(2).copied().unwrap_or(0));
        for position in 0..4 {
            if position <= chunk.len() {
                let index = (triple >> (18 - 6 * position)) & 63;
                out.push(char::from(ALPHABET[index as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("project");
        std::fs::write(dir.path().join("project.godot"), "config_version=5\n").expect("project");
        for sub in ["zones", "zones/far", "zones/.hidden", "other"] {
            std::fs::create_dir_all(dir.path().join(sub)).expect(sub);
        }
        for file in [
            "zones/zone_0_0.tscn",
            "zones/zone_0_1.tscn",
            "zones/zone_1_0.scn",
            "zones/notes.txt",
            "zones/lake.tscn",
            "zones/far/zone_9_9.tscn",
            "zones/.hidden/zone_5_5.tscn",
            "other/rock.tscn",
            "other/sky.tres",
        ] {
            std::fs::write(dir.path().join(file), "").expect(file);
        }
        dir
    }

    #[test]
    fn dir_and_glob_find_sorted_scenes_and_skip_what_godot_skips() {
        let dir = project();
        let request = map_request(dir.path(), &json!({"dir": "res://zones", "glob": "zone_*"}))
            .expect("request");
        assert_eq!(
            request["scenes"],
            json!([
                "res://zones/zone_0_0.tscn",
                "res://zones/zone_0_1.tscn",
                "res://zones/zone_1_0.scn"
            ])
        );
        let recursive = map_request(
            dir.path(),
            &json!({"dir": "zones", "glob": "zone_?_?.tscn", "recursive": true, "scenes": ["res://other/rock.tscn", "res://zones/zone_0_0.tscn"]}),
        )
        .expect("request");
        assert_eq!(
            recursive["scenes"],
            json!([
                "res://other/rock.tscn",
                "res://zones/far/zone_9_9.tscn",
                "res://zones/zone_0_0.tscn",
                "res://zones/zone_0_1.tscn"
            ]),
            "hidden directories are skipped and duplicates merged"
        );
        let defaults = map_request(dir.path(), &json!({"dir": "res://zones"})).expect("request");
        assert_eq!(defaults["scenes"].as_array().map(Vec::len), Some(3));
        assert_eq!(defaults["view"], "geometry");
        assert_eq!(defaults["labels"], true);
        assert_eq!(defaults["default_side"], 2048.0);
        assert!(defaults.get("rect").is_none(), "{defaults}");
    }

    #[test]
    fn paths_stay_in_the_project_and_name_scene_files() {
        let dir = project();
        for (args, expected) in [
            (json!({"dir": "res://../zones"}), "dir:"),
            (json!({"dir": "res://missing"}), "no such directory"),
            (
                json!({"dir": "res://zones", "glob": "far/*.tscn"}),
                "without /",
            ),
            (
                json!({"dir": "res://zones", "glob": "*.png"}),
                "matches *.png",
            ),
            (json!({"glob": "*.tscn"}), "need dir"),
            (
                json!({"scenes": ["res://other/sky.tres"]}),
                "scenes[0] must be a .tscn",
            ),
            (
                json!({"scenes": ["res://other/none.tscn"]}),
                "scenes[0]: no such file",
            ),
            (json!({"scenes": ["../outside.tscn"]}), "scenes[0]:"),
            (json!({}), "give dir"),
        ] {
            let problem = map_request(dir.path(), &args).expect_err(&args.to_string());
            assert!(problem.contains(expected), "{args}: {problem}");
        }
    }

    #[test]
    fn numbers_are_checked_and_the_map_size_is_bounded() {
        let dir = project();
        let base = |extra: Value| {
            let mut args = json!({"scenes": ["res://other/rock.tscn"]});
            for (key, value) in extra.as_object().expect("object") {
                args[key] = value.clone();
            }
            map_request(dir.path(), &args)
        };
        let ok = base(json!({"rect": [-480, -480, 480, 480], "pixels_per_metre": 2, "view": "collision", "labels": false, "tile_metres": 96})).expect("request");
        assert_eq!(ok["rect"], json!([-480.0, -480.0, 480.0, 480.0]));
        assert_eq!(ok["pixels_per_metre"], 2.0);
        assert_eq!(ok["view"], "collision");
        assert_eq!(ok["labels"], false);
        assert_eq!(ok["tile_metres"], 96.0);
        for (extra, expected) in [
            (json!({"rect": [0, 0, 10]}), "rect must be"),
            (json!({"rect": [10, 0, 0, 10]}), "max above min"),
            (json!({"pixels_per_metre": 0}), "pixels_per_metre"),
            (
                json!({"rect": [0, 0, 1000, 10], "pixels_per_metre": 8}),
                "4.096 or less",
            ),
            (json!({"view": "wire"}), "view must be"),
            (json!({"labels": "yes"}), "labels must be"),
            (json!({"tile_metres": 0.5}), "tile_metres"),
        ] {
            let problem = base(extra.clone()).expect_err(&extra.to_string());
            assert!(problem.contains(expected), "{extra}: {problem}");
        }
    }

    #[test]
    fn too_many_scenes_are_refused() {
        let dir = tempfile::tempdir().expect("project");
        std::fs::create_dir_all(dir.path().join("zones")).expect("zones");
        for index in 0..=MAX_SCENES {
            std::fs::write(dir.path().join(format!("zones/z{index}.tscn")), "").expect("scene");
        }
        let problem =
            map_request(dir.path(), &json!({"dir": "res://zones"})).expect_err("too many");
        assert!(problem.contains("more than 256"), "{problem}");
    }

    #[test]
    fn wildcards_match_whole_names() {
        assert!(wildcard_match("zone_*.tscn", "zone_3_4.tscn"));
        assert!(wildcard_match("*", "a"));
        assert!(wildcard_match("z?ne_*_*.tscn", "zone_10_2.tscn"));
        assert!(wildcard_match("*_4.tscn", "zone_3_4.tscn"));
        assert!(!wildcard_match("zone_*.tscn", "zone_3_4.tscn.bak"));
        assert!(!wildcard_match("zone_?.tscn", "zone_10.tscn"));
        assert!(!wildcard_match("lake*", "zone_lake.tscn"));
    }

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(&[0xff, 0xfe, 0x00]), "//4A");
    }
}
