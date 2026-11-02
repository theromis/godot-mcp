//! Names the commit the server is built from, so a build between two releases reports more than
//! the manifest version (see `src/version.rs`). A build without git, or from a source archive
//! that has no `.git`, simply has no commit to name.

use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8(output.stdout).ok())
        .flatten()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GODOT_MCP_BUILD");

    // A commit given by the environment wins (a build from a tarball can pass it).
    let commit = std::env::var("GODOT_MCP_BUILD")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| git(&["rev-parse", "--short=9", "HEAD"]));
    if let Some(commit) = commit {
        println!("cargo:rustc-env=GODOT_MCP_BUILD={commit}");
    }

    // Build again when HEAD moves: its file, and the branch files it points to. `--git-path`
    // finds them in a worktree too, where `.git` is a file.
    for name in ["HEAD", "refs/heads", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            let path = PathBuf::from(path);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
