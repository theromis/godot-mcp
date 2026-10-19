//! Submission of a feedback draft to the project's issue tracker through the human's own
//! signed-in GitHub command-line client. The server stores no credentials and calls no web API
//! itself; whatever `gh` is signed in as is what publishes the issue.

use crate::feedback::{ScrubContext, scrub};
use crate::journal::Outcome;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(20);
const CREATE_TIMEOUT: Duration = Duration::from_secs(45);
const QUOTED_LINE_LIMIT: usize = 200;
const BY_HAND: &str =
    "Or file it by hand: give the human the report text, or the prefilled link from the draft";

/// The repository the product is published in, as named by the package manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoAddress {
    owner: String,
    name: String,
}

impl RepoAddress {
    /// Reads `https://github.com/{owner}/{name}` (also with a trailing `/` or `.git`).
    pub fn parse(url: &str) -> Option<Self> {
        let rest = url.trim().strip_prefix("https://github.com/")?;
        let rest = rest.trim_end_matches('/');
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        let (owner, name) = rest.split_once('/')?;
        let valid = |part: &str| {
            !part.is_empty()
                && !part.starts_with('-')
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        (valid(owner) && valid(name)).then(|| Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    /// The repository named by this crate's own manifest.
    pub fn manifest() -> Option<Self> {
        Self::parse(env!("CARGO_PKG_REPOSITORY"))
    }

    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    pub fn new_issue_url(&self) -> String {
        format!("{}new", self.issue_prefix())
    }

    /// What every issue address in this repository starts with.
    pub fn issue_prefix(&self) -> String {
        format!("https://github.com/{}/{}/issues/", self.owner, self.name)
    }
}

/// Where the GitHub client is: the configured path when set (it must exist), else `gh` on the
/// `PATH`. Looking does not start anything.
pub fn locate_gh(configured: Option<&Path>) -> Option<PathBuf> {
    locate_gh_in(configured, std::env::var_os("PATH").as_deref())
}

fn locate_gh_in(configured: Option<&Path>, path_var: Option<&OsStr>) -> Option<PathBuf> {
    if let Some(configured) = configured.filter(|path| !path.as_os_str().is_empty()) {
        return configured.is_file().then(|| configured.to_path_buf());
    }
    let name = if cfg!(windows) { "gh.exe" } else { "gh" };
    std::env::split_paths(path_var?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn list_args(repo: &RepoAddress, fingerprint: &str) -> Vec<String> {
    [
        "issue",
        "list",
        "--repo",
        &repo.slug(),
        "--state",
        "all",
        "--search",
        &format!("{fingerprint} in:body"),
        "--json",
        "number,title,url,body",
        "--limit",
        "5",
    ]
    .map(String::from)
    .to_vec()
}

/// The title is one `--title=` argument, so a title that starts with `-` can never be read as an
/// option. The body goes through standard input, never through the command line.
fn create_args(repo: &RepoAddress, title: &str) -> Vec<String> {
    [
        "issue",
        "create",
        "--repo",
        &repo.slug(),
        &format!("--title={title}"),
        "--body-file",
        "-",
    ]
    .map(String::from)
    .to_vec()
}

/// Finds the issue, among the search hits, whose body carries the fingerprint. A hit counts only
/// when its address is an issue of this repository.
fn parse_duplicates(
    json: &str,
    fingerprint: &str,
    repo: &RepoAddress,
) -> Result<Option<String>, String> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|err| format!("the issue search answered with something unreadable ({err})"))?;
    let hits = value
        .as_array()
        .ok_or_else(|| "the issue search did not answer with a list".to_string())?;
    let prefix = repo.issue_prefix();
    Ok(hits.iter().find_map(|hit| {
        let body = hit.get("body")?.as_str()?;
        let url = hit.get("url")?.as_str()?;
        (body.contains(fingerprint) && url.starts_with(&prefix)).then(|| url.to_string())
    }))
}

/// Maps what `gh issue create` did to one of the outcomes the channel reports.
fn interpret_create(success: bool, stdout: &str, stderr: &str, repo: &RepoAddress) -> Outcome {
    if success {
        let prefix = repo.issue_prefix();
        let address = stdout
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .filter(|line| {
                line.strip_prefix(&prefix).is_some_and(|number| {
                    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
                })
            });
        return match address {
            Some(address) => Outcome::Created(address.to_string()),
            None => Outcome::Unknown(
                "the GitHub CLI reported success but printed no issue address".to_string(),
            ),
        };
    }

    let shown = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let lower = shown.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|needle| lower.contains(needle));
    let (reason, first_step) = if any(&[
        "gh auth login",
        "not logged in",
        "gh_token",
        "authentication",
    ]) {
        (
            "the GitHub CLI is not signed in".to_string(),
            "Run `gh auth login` in a terminal, then submit again",
        )
    } else if any(&["rate limit"]) {
        (
            "GitHub is limiting requests".to_string(),
            "Wait a few minutes, then submit again",
        )
    } else if any(&[
        "could not resolve host",
        "dial tcp",
        "no such host",
        "connection refused",
        "network is unreachable",
        "i/o timeout",
        "tls handshake",
    ]) {
        (
            "the GitHub CLI could not reach GitHub".to_string(),
            "Check the network connection, then submit again",
        )
    } else {
        (
            format!("the GitHub CLI reported: {}", first_line(shown)),
            "Fix the cause named above, then submit again",
        )
    };
    Outcome::NotCreated {
        reason,
        steps: vec![first_step.to_string(), BY_HAND.to_string()],
    }
}

/// The first non-empty line of `text`, scrubbed and cut, for quoting in an answer.
fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no message");
    scrub(line, &ScrubContext::default())
        .chars()
        .take(QUOTED_LINE_LIMIT)
        .collect()
}

enum RunFailure {
    Spawn(String),
    TimedOut,
    Io(String),
}

struct Finished {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Runs the GitHub client for one feedback submission.
pub struct IssueClient {
    gh: PathBuf,
    repo: RepoAddress,
    lookup_limit: Duration,
    create_limit: Duration,
}

impl IssueClient {
    pub fn new(gh: PathBuf, repo: RepoAddress) -> Self {
        Self {
            gh,
            repo,
            lookup_limit: LOOKUP_TIMEOUT,
            create_limit: CREATE_TIMEOUT,
        }
    }

    /// Overrides how long the look-up and the creation may take.
    pub fn with_limits(mut self, lookup: Duration, create: Duration) -> Self {
        self.lookup_limit = lookup;
        self.create_limit = create;
        self
    }

    /// The address of an issue that already carries `fingerprint`, if there is one. An error is
    /// a failed look-up, which the caller reports and then carries on.
    pub async fn find_duplicate(&self, fingerprint: &str) -> Result<Option<String>, String> {
        let args = list_args(&self.repo, fingerprint);
        match self.run(&args, None, self.lookup_limit).await {
            Ok(finished) if finished.success => {
                parse_duplicates(&finished.stdout, fingerprint, &self.repo)
            }
            Ok(finished) => Err(first_line(if finished.stderr.trim().is_empty() {
                &finished.stdout
            } else {
                &finished.stderr
            })),
            Err(RunFailure::Spawn(detail) | RunFailure::Io(detail)) => Err(detail),
            Err(RunFailure::TimedOut) => Err(format!(
                "the issue search did not finish within {} s",
                self.lookup_limit.as_secs()
            )),
        }
    }

    /// Creates the issue. A client that could not even start created nothing; one that did not
    /// answer in time may have, so that case is reported as unknown.
    pub async fn create(&self, title: &str, body: &str) -> Outcome {
        let args = create_args(&self.repo, title);
        match self.run(&args, Some(body), self.create_limit).await {
            Ok(finished) => {
                interpret_create(finished.success, &finished.stdout, &finished.stderr, &self.repo)
            }
            Err(RunFailure::Spawn(detail)) => Outcome::NotCreated {
                reason: format!("the GitHub CLI could not be started: {}", first_line(&detail)),
                steps: vec![
                    "Install the GitHub CLI (https://cli.github.com) and sign in with `gh auth login`"
                        .to_string(),
                    "Or set GODOT_MCP_GH_PATH to the full path of the gh executable".to_string(),
                    BY_HAND.to_string(),
                ],
            },
            Err(RunFailure::TimedOut) => Outcome::Unknown(format!(
                "the GitHub CLI did not answer within {} s; the issue may or may not exist",
                self.create_limit.as_secs()
            )),
            Err(RunFailure::Io(detail)) => Outcome::Unknown(format!(
                "talking to the GitHub CLI failed ({})",
                first_line(&detail)
            )),
        }
    }

    async fn run(
        &self,
        args: &[String],
        stdin: Option<&str>,
        limit: Duration,
    ) -> Result<Finished, RunFailure> {
        let mut command = Command::new(&self.gh);
        command
            .args(args)
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .env("GH_NO_UPDATE_NOTIFIER", "1")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // The server's own standard handles carry the protocol; the client must not inherit them.
        #[cfg(windows)]
        crate::cli_runner::make_std_handles_private();

        let mut child = command
            .spawn()
            .map_err(|err| RunFailure::Spawn(err.to_string()))?;
        if let (Some(body), Some(mut input)) = (stdin, child.stdin.take()) {
            let body = body.to_owned();
            // A client that exits before reading its input is judged by its exit status alone.
            tokio::spawn(async move {
                let _ = input.write_all(body.as_bytes()).await;
                let _ = input.shutdown().await;
            });
        }
        let output = timeout(limit, child.wait_with_output())
            .await
            .map_err(|_| RunFailure::TimedOut)?
            .map_err(|err| RunFailure::Io(err.to_string()))?;
        Ok(Finished {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> RepoAddress {
        RepoAddress::parse("https://github.com/zeegzoog/godot-mcp").expect("repository")
    }

    #[test]
    fn the_repository_address_is_read_from_the_manifest_url() {
        for url in [
            "https://github.com/zeegzoog/godot-mcp",
            "https://github.com/zeegzoog/godot-mcp.git",
            "https://github.com/zeegzoog/godot-mcp/",
            "  https://github.com/zeegzoog/godot-mcp  ",
        ] {
            let parsed = RepoAddress::parse(url).unwrap_or_else(|| panic!("{url}"));
            assert_eq!(parsed.slug(), "zeegzoog/godot-mcp", "{url}");
        }
        let parsed = repo();
        assert_eq!(
            parsed.new_issue_url(),
            "https://github.com/zeegzoog/godot-mcp/issues/new"
        );
        assert_eq!(
            parsed.issue_prefix(),
            "https://github.com/zeegzoog/godot-mcp/issues/"
        );
    }

    #[test]
    fn anything_that_is_not_a_github_repository_url_is_rejected() {
        for url in [
            "",
            "https://gitlab.com/o/r",
            "http://github.com/o/r",
            "https://github.com/onlyowner",
            "https://github.com/o/r/tree/main",
            "https://github.com//r",
            "https://github.com/o/ r",
            "https://github.com/-o/r",
        ] {
            assert!(RepoAddress::parse(url).is_none(), "{url}");
        }
    }

    #[test]
    fn the_package_manifest_names_a_usable_repository() {
        let manifest = RepoAddress::manifest().expect("the manifest repository parses");
        assert!(manifest.slug().contains('/'));
        assert!(manifest.new_issue_url().ends_with("/issues/new"));
    }

    #[test]
    fn the_look_up_arguments_are_fixed_and_carry_no_free_text() {
        assert_eq!(
            list_args(&repo(), "gmfb0123456789ab"),
            [
                "issue",
                "list",
                "--repo",
                "zeegzoog/godot-mcp",
                "--state",
                "all",
                "--search",
                "gmfb0123456789ab in:body",
                "--json",
                "number,title,url,body",
                "--limit",
                "5",
            ]
        );
    }

    #[test]
    fn the_title_travels_as_one_argument_that_cannot_be_read_as_an_option() {
        let args = create_args(&repo(), "--web is not an option");
        assert_eq!(
            args,
            [
                "issue",
                "create",
                "--repo",
                "zeegzoog/godot-mcp",
                "--title=--web is not an option",
                "--body-file",
                "-",
            ]
        );
        assert!(
            !args.iter().any(|arg| arg.starts_with("--label")),
            "no label option"
        );
    }

    #[test]
    fn a_created_issue_is_recognized_by_its_address_on_the_last_line() {
        let outcome = interpret_create(
            true,
            "\nCreating issue in zeegzoog/godot-mcp\n\nhttps://github.com/zeegzoog/godot-mcp/issues/42\n\n",
            "",
            &repo(),
        );
        assert!(
            matches!(&outcome, Outcome::Created(address) if address == "https://github.com/zeegzoog/godot-mcp/issues/42"),
            "{outcome:?}"
        );
    }

    #[test]
    fn success_without_an_issue_address_is_unknown_not_created() {
        for stdout in [
            "",
            "Created!",
            "https://github.com/zeegzoog/godot-mcp/issues/",
            "https://github.com/zeegzoog/godot-mcp/issues/abc",
            "https://github.com/other/repo/issues/7",
            "https://github.com/zeegzoog/godot-mcp/pull/7",
        ] {
            let outcome = interpret_create(true, stdout, "", &repo());
            assert!(
                matches!(outcome, Outcome::Unknown(_)),
                "{stdout:?}: {outcome:?}"
            );
        }
    }

    fn not_created(stderr: &str) -> (String, Vec<String>) {
        match interpret_create(false, "", stderr, &repo()) {
            Outcome::NotCreated { reason, steps } => (reason, steps),
            other => panic!("expected NotCreated, got {other:?}"),
        }
    }

    #[test]
    fn a_failure_names_what_went_wrong_and_what_to_do() {
        let (reason, steps) =
            not_created("To get started with GitHub CLI, please run:  gh auth login");
        assert!(reason.contains("not signed in"), "{reason}");
        assert!(steps[0].contains("gh auth login"), "{steps:?}");

        let (reason, _) = not_created(
            "gh: To use GitHub CLI in automation, set the GH_TOKEN environment variable.",
        );
        assert!(reason.contains("not signed in"), "{reason}");

        let (reason, steps) = not_created(
            "error connecting to api.github.com: dial tcp: lookup api.github.com: no such host",
        );
        assert!(reason.contains("reach GitHub"), "{reason}");
        assert!(steps[0].contains("network"), "{steps:?}");

        let (reason, _) = not_created("API rate limit exceeded for user");
        assert!(reason.contains("limiting"), "{reason}");

        let (reason, steps) = not_created("\nGraphQL: Something unexpected (createIssue)\nmore");
        assert!(reason.contains("Something unexpected"), "{reason}");
        assert!(
            !reason.contains("more"),
            "only the first line is quoted: {reason}"
        );
        assert!(!steps.is_empty());
    }

    #[test]
    fn every_failure_also_points_to_the_hand_filing_path() {
        for stderr in ["gh auth login", "no such host", "rate limit", "boom"] {
            let (_, steps) = not_created(stderr);
            assert!(
                steps.iter().any(|step| step.contains("by hand")),
                "{stderr}: {steps:?}"
            );
        }
    }

    #[test]
    fn a_quoted_failure_line_is_scrubbed() {
        let (reason, _) = not_created("cannot read /home/alice/.config/gh/hosts.yml");
        assert!(
            reason.contains("{path}") && !reason.contains("alice"),
            "{reason}"
        );
    }

    #[test]
    fn a_duplicate_counts_only_when_the_body_holds_the_fingerprint() {
        let json = r#"[
            {"number": 3, "title": "other", "url": "https://github.com/zeegzoog/godot-mcp/issues/3", "body": "unrelated"},
            {"number": 9, "title": "same", "url": "https://github.com/zeegzoog/godot-mcp/issues/9", "body": "text\n- Fingerprint: gmfb0123456789ab\n"}
        ]"#;
        assert_eq!(
            parse_duplicates(json, "gmfb0123456789ab", &repo()).expect("parsed"),
            Some("https://github.com/zeegzoog/godot-mcp/issues/9".to_string())
        );
        assert_eq!(
            parse_duplicates(json, "gmfbffffffffffff", &repo()).expect("parsed"),
            None
        );
        assert_eq!(
            parse_duplicates("[]", "gmfb0123456789ab", &repo()).expect("parsed"),
            None
        );
    }

    #[test]
    fn a_duplicate_with_a_foreign_address_is_ignored_and_bad_json_is_an_error() {
        let foreign = r#"[{"number": 1, "title": "t", "url": "https://evil.example/issues/1", "body": "gmfb0123456789ab"}]"#;
        assert_eq!(
            parse_duplicates(foreign, "gmfb0123456789ab", &repo()).expect("parsed"),
            None
        );
        assert!(parse_duplicates("not json", "gmfb0123456789ab", &repo()).is_err());
        assert!(parse_duplicates("{}", "gmfb0123456789ab", &repo()).is_err());
    }

    #[test]
    fn the_client_is_found_where_it_is_configured_or_on_the_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let name = if cfg!(windows) { "gh.exe" } else { "gh" };
        let file = dir.path().join(name);
        std::fs::write(&file, "").expect("write");

        assert_eq!(locate_gh_in(Some(&file), None), Some(file.clone()));
        assert_eq!(locate_gh_in(Some(&dir.path().join("missing")), None), None);
        let path_var = std::env::join_paths([dir.path()]).expect("path var");
        assert_eq!(locate_gh_in(None, Some(path_var.as_os_str())), Some(file));
        let empty = tempfile::tempdir().expect("temp dir");
        let other = std::env::join_paths([empty.path()]).expect("path var");
        assert_eq!(locate_gh_in(None, Some(other.as_os_str())), None);
        assert_eq!(locate_gh_in(None, None), None);
    }

    #[test]
    fn an_empty_configured_path_counts_as_not_set() {
        assert_eq!(locate_gh_in(Some(std::path::Path::new("")), None), None);
    }
}
