//! Godot's exit-time leak reports, taken out of a headless run's output and counted.
//!
//! A SceneTree script that loads many scenes ends with hundreds of lines Godot prints while it
//! shuts down (`Leaked instance dependency`, `N RID allocations of type ... were leaked at exit`,
//! ...). They say nothing about what the script did, and they bury the errors that do, so the
//! answer drops them and counts them by kind instead.

use std::collections::BTreeMap;

/// The exit-time reports, by the name `exit_leaks` gives them, and how each is recognised in the
/// text after Godot's `ERROR: ` or `WARNING: ` prefix.
/// Whether the text of a message is one kind of exit-time report.
type Matcher = fn(&str) -> bool;

const KINDS: &[(&str, Matcher)] = &[
    ("leaked_instance_dependency", |text| {
        text.starts_with("Leaked instance dependency:")
    }),
    ("rid_allocations", |text| {
        text.contains(" RID allocations of type ") && text.contains("were leaked at exit")
    }),
    ("objectdb_instances", |text| {
        text.contains(" ObjectDB instance") && text.contains("leaked at exit")
    }),
    ("resources_in_use", |text| {
        text.contains("resources still in use at exit")
    }),
    ("paged_allocator", |text| {
        text.starts_with("Pages in use exist at exit in PagedAllocator")
    }),
];

/// The kind of exit-time report `line` starts, if it starts one.
fn kind_of(line: &str) -> Option<&'static str> {
    let plain = strip_ansi(line);
    let trimmed = plain.trim();
    let text = trimmed
        .strip_prefix("ERROR:")
        .or_else(|| trimmed.strip_prefix("WARNING:"))?
        .trim_start();
    KINDS
        .iter()
        .find(|(_, matches)| matches(text))
        .map(|(kind, _)| *kind)
}

fn is_source_line(line: &str) -> bool {
    strip_ansi(line).trim_start().starts_with("at:")
}

/// `text` without the exit-time reports (and the `at:` source line Godot prints under each),
/// with every other line exactly as it was. Each dropped report is counted in `counts`.
pub(super) fn strip_exit_leaks(text: &str, counts: &mut BTreeMap<&'static str, usize>) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut after_report = false;
    for line in text.split_inclusive('\n') {
        if let Some(kind) = kind_of(line) {
            *counts.entry(kind).or_insert(0) += 1;
            after_report = true;
        } else if after_report && is_source_line(line) {
            after_report = false;
        } else {
            after_report = false;
            kept.push_str(line);
        }
    }
    kept
}

fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            plain.push(c);
            continue;
        }
        // CSI: ESC [ parameters, ended by a letter.
        if chars.next() == Some('[') {
            for end in chars.by_ref() {
                if end.is_ascii_alphabetic() {
                    break;
                }
            }
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHUTDOWN: &str = "\
WARNING: Leaked instance dependency: Bug - did not call instance_notify_deleted when freeing.
     at: ~DependencyTracker (servers/rendering/renderer_rd/storage_rd/utilities.h:79)
WARNING: Leaked instance dependency: Bug - did not call instance_notify_deleted when freeing.
     at: ~DependencyTracker (servers/rendering/renderer_rd/storage_rd/utilities.h:79)
WARNING: 5 ObjectDB instances were leaked at exit (run with `--verbose` for details).
   at: cleanup (core/object/object.cpp:2609)
ERROR: 3 resources still in use at exit (run with --verbose for details).
   at: clear (core/io/resource.cpp:599)
ERROR: 7 RID allocations of type 'N12MeshStorage4MeshE' were leaked at exit.
ERROR: Pages in use exist at exit in PagedAllocator: N7Skeleton
   at: ~PagedAllocator (./core/templates/paged_allocator.h:170)
";

    #[test]
    fn drops_each_shutdown_report_and_its_source_line_and_counts_them() {
        let mut counts = BTreeMap::new();
        let text = format!("placed 12 rocks\n{SHUTDOWN}");
        assert_eq!(strip_exit_leaks(&text, &mut counts), "placed 12 rocks\n");
        assert_eq!(
            counts,
            BTreeMap::from([
                ("leaked_instance_dependency", 2),
                ("objectdb_instances", 1),
                ("paged_allocator", 1),
                ("resources_in_use", 1),
                ("rid_allocations", 1),
            ])
        );
    }

    #[test]
    fn keeps_every_other_error_and_warning_as_it_was() {
        let other = "\
ERROR: Failed to load resource 'res://zones/missing.tscn'.
   at: _load (core/io/resource_loader.cpp:283)
WARNING: Node name has invalid characters
   at: set_name (scene/main/node.cpp:1300)
SCRIPT ERROR: Invalid call. Nonexistent function 'foo'.
   at: _initialize (res://place.gd:14)
leaked at exit, said the script
";
        let mut counts = BTreeMap::new();
        let text = format!("{SHUTDOWN}{other}");
        assert_eq!(strip_exit_leaks(&text, &mut counts), other);
        assert_eq!(counts.values().sum::<usize>(), 6);
    }

    #[test]
    fn a_source_line_stays_when_no_report_comes_before_it() {
        let mut counts = BTreeMap::new();
        let text =
            "ERROR: 2 RID allocations of type 'X' were leaked at exit.\nok\n   at: f (a.cpp:1)";
        assert_eq!(
            strip_exit_leaks(text, &mut counts),
            "ok\n   at: f (a.cpp:1)"
        );
        assert_eq!(counts, BTreeMap::from([("rid_allocations", 1)]));
    }

    #[test]
    fn recognises_colored_reports() {
        let mut counts = BTreeMap::new();
        let text = "\u{1b}[1;33mWARNING:\u{1b}[0;93m 1 ObjectDB instance was leaked at exit (run with `--verbose` for details).\u{1b}[0m\n\u{1b}[0;90m     at: cleanup (core/object/object.cpp:2577)\u{1b}[0m\nnext\n";
        assert_eq!(strip_exit_leaks(text, &mut counts), "next\n");
        assert_eq!(counts, BTreeMap::from([("objectdb_instances", 1)]));
    }
}
