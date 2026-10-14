//! Path and identifier checks shared by the tools that touch the file system.

use std::path::{Component, Path, PathBuf};

const RES_PREFIX: &str = "res://";

/// Matches `^[A-Za-z_][A-Za-z0-9_]*$`, the only shape accepted for class and node identifiers.
pub fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

/// Checks that `dir` exists and is the root of a Godot project.
pub fn ensure_project_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        return Err(format!(
            "Project directory does not exist: {}",
            dir.display()
        ));
    }
    if !dir.join("project.godot").is_file() {
        return Err(format!(
            "Not a Godot project directory (project.godot not found): {}",
            dir.display()
        ));
    }
    Ok(())
}

/// Resolves a `res://` path, a project-relative path, or an absolute path to a location
/// inside `project`.
///
/// Rejects `..` segments (a name merely containing two dots is fine) and anything that lands
/// outside the project, including through symlinks of the part that already exists.
pub fn resolve_in_project(project: &Path, requested: &str) -> Result<PathBuf, String> {
    let trimmed = requested.trim();
    if trimmed.is_empty() {
        return Err("Path is empty".into());
    }

    let relative = trimmed.strip_prefix(RES_PREFIX).unwrap_or(trimmed);
    let normalized = relative.replace('\\', "/");
    let path = Path::new(&normalized);

    let is_absolute = path.has_root()
        || path
            .components()
            .any(|component| matches!(component, Component::Prefix(_)));
    let full = if is_absolute {
        path.to_path_buf()
    } else {
        project.join(path)
    };

    let mut resolved = PathBuf::new();
    for component in full.components() {
        match component {
            Component::ParentDir => {
                return Err(format!(
                    "Parent directory segments ('..') are not allowed: {requested}"
                ));
            }
            Component::CurDir => {}
            other => resolved.push(other.as_os_str()),
        }
    }

    let project_root = project.canonicalize().map_err(|err| {
        format!(
            "Cannot access project directory {}: {err}",
            project.display()
        )
    })?;
    let existing_ancestor = resolved
        .ancestors()
        .find(|candidate| candidate.exists())
        .and_then(|candidate| candidate.canonicalize().ok());
    match existing_ancestor {
        Some(ancestor) if ancestor.starts_with(&project_root) => Ok(resolved),
        _ => Err(format!(
            "Path is outside the project directory {}: {requested}",
            project.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("project.godot"), "config_version=5\n").expect("write");
        dir
    }

    #[test]
    fn identifiers_follow_the_strict_pattern() {
        for valid in ["Node", "_private", "Player2D", "a"] {
            assert!(is_valid_identifier(valid), "{valid}");
        }
        for invalid in ["", "2D", "Foo Bar", "Foo\nBar", "a-b", "Node;", "ä"] {
            assert!(!is_valid_identifier(invalid), "{invalid:?}");
        }
    }

    #[test]
    fn res_prefix_and_backslashes_resolve_under_the_project() {
        let dir = project();
        let resolved = resolve_in_project(dir.path(), "res://scripts\\player.gd").expect("resolve");
        assert_eq!(resolved, dir.path().join("scripts").join("player.gd"));
    }

    #[test]
    fn parent_segments_are_rejected_but_double_dots_in_names_are_not() {
        let dir = project();
        assert!(resolve_in_project(dir.path(), "a/../b.gd").is_err());
        assert!(resolve_in_project(dir.path(), "../b.gd").is_err());
        assert!(resolve_in_project(dir.path(), "my..file.gd").is_ok());
    }

    #[test]
    fn absolute_paths_must_stay_inside_the_project() {
        let dir = project();
        let outside = tempfile::tempdir().expect("outside");

        let inside_file = dir.path().join("sub").join("a.gd");
        assert!(resolve_in_project(dir.path(), &inside_file.to_string_lossy()).is_ok());

        let outside_file = outside.path().join("a.gd");
        assert!(resolve_in_project(dir.path(), &outside_file.to_string_lossy()).is_err());
    }

    #[test]
    fn empty_paths_are_rejected() {
        let dir = project();
        assert!(resolve_in_project(dir.path(), "  ").is_err());
    }

    #[test]
    fn project_directory_needs_project_godot() {
        let dir = project();
        assert!(ensure_project_dir(dir.path()).is_ok());

        let empty = tempfile::tempdir().expect("empty");
        let err = ensure_project_dir(empty.path()).expect_err("no project.godot");
        assert!(err.contains("project.godot"));

        let missing = empty.path().join("missing");
        assert!(ensure_project_dir(&missing).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_be_used_to_leave_the_project() {
        let dir = project();
        let outside = tempfile::tempdir().expect("outside");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).expect("symlink");
        assert!(resolve_in_project(dir.path(), "link/a.gd").is_err());
    }
}
