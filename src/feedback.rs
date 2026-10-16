//! The agent feedback channel: what an agent noticed while working, turned into one scrubbed
//! report for the maintainers. Everything here lives in memory for the life of the process and
//! nothing is sent anywhere by this module.

use std::path::PathBuf;

/// Largest prefilled new-issue link, in bytes. GitHub does not publish its limit (an over-long
/// link answers 414), so this stays well below the 8 KB most servers accept.
pub const LINK_BUDGET: usize = 6_000;

/// Ends a link whose body had to be cut.
pub const LINK_CUT_MARKER: &str =
    "\n\n... (shortened; paste the full report from the conversation)";

const REDACTED: &str = "{redacted}";
const PROJECT: &str = "{project}";
const PATH: &str = "{path}";
const HOME: &str = "~";
const ZERO_WIDTH_SPACE: char = '\u{200B}';

/// Prefixes of well-known credential formats, each followed by at least `MIN_AFTER_PREFIX`
/// characters when it is a credential.
const CREDENTIAL_PREFIXES: [&str; 8] = [
    "ghp_",
    "gho_",
    "ghs_",
    "github_pat_",
    "sk-",
    "xoxb-",
    "xoxp-",
    "AKIA",
];
const MIN_AFTER_PREFIX: usize = 8;
const LONG_RUN: usize = 32;
const UNIX_ROOTS: [&str; 7] = ["home", "Users", "root", "tmp", "var", "opt", "mnt"];

/// What the scrubber must remove besides the patterns it recognizes by shape.
#[derive(Debug, Default, Clone)]
pub struct ScrubContext {
    /// The bridge secret, removed wherever it appears.
    pub secret: Option<String>,
    /// The project directory, shown as `{project}`.
    pub project_dir: Option<PathBuf>,
    /// The user's home directory, shown as `~`.
    pub home_dir: Option<PathBuf>,
}

impl ScrubContext {
    /// A context whose home directory comes from the environment.
    pub fn from_environment(secret: Option<String>, project_dir: Option<PathBuf>) -> Self {
        let home_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let home_dir = std::env::var_os(home_variable)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            secret,
            project_dir,
            home_dir,
        }
    }
}

/// Removes everything from `text` that must not reach a public tracker: the bridge secret,
/// credentials, absolute filesystem paths and Markdown that notifies users or embeds content.
/// A second pass over the result changes nothing.
pub fn scrub(text: &str, context: &ScrubContext) -> String {
    let mut text = strip_control(text);
    if let Some(secret) = context
        .secret
        .as_deref()
        .filter(|secret| !secret.is_empty())
    {
        text = text.replace(secret, REDACTED);
    }
    if let Some(dir) = &context.project_dir {
        text = replace_directory(&text, &dir.to_string_lossy(), PROJECT, cfg!(windows));
    }
    // The home directory is shortened only once the project directory is known: a project that
    // lives under the home directory would otherwise show its own folder name as `~/Name/...`.
    // Without it, such paths are left to the absolute-path step, which hides them completely.
    if let (Some(_), Some(dir)) = (&context.project_dir, &context.home_dir) {
        text = replace_directory(&text, &dir.to_string_lossy(), HOME, cfg!(windows));
    }
    let text = redact_paths(&text);
    let text = redact_credentials(&text);
    neutralize_markdown(&text)
}

/// Drops control characters (newline and tab stay) and the bidirectional override characters
/// that can make text read differently from how it is stored.
fn strip_control(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            matches!(c, '\n' | '\t')
                || !(c.is_control()
                    || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
        })
        .collect()
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Replaces `dir`, written with either separator, by `placeholder`. A match that continues into
/// a longer name (`/game` inside `/game2`) is not a match.
fn replace_directory(text: &str, dir: &str, placeholder: &str, ignore_case: bool) -> String {
    let dir = dir.trim_end_matches(['/', '\\']);
    if !dir.contains(['/', '\\']) {
        return text.to_string();
    }
    let spellings = [
        dir.to_string(),
        dir.replace('\\', "/"),
        dir.replace('/', "\\"),
    ];
    spellings.iter().fold(text.to_string(), |text, spelling| {
        replace_bounded(&text, spelling, placeholder, ignore_case)
    })
}

fn replace_bounded(text: &str, needle: &str, replacement: &str, ignore_case: bool) -> String {
    // ASCII lowercasing keeps every byte offset, so positions found in the folded copy are
    // valid in the original.
    let (haystack, pattern) = if ignore_case {
        (text.to_ascii_lowercase(), needle.to_ascii_lowercase())
    } else {
        (text.to_string(), needle.to_string())
    };
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(found) = haystack[cursor..].find(&pattern) {
        let start = cursor + found;
        let end = start + pattern.len();
        let separated_before = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_name_char(c));
        let separated_after = text[end..].chars().next().is_none_or(|c| !is_name_char(c));
        out.push_str(&text[cursor..start]);
        if separated_before && separated_after {
            out.push_str(replacement);
        } else {
            out.push_str(&text[start..end]);
        }
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

fn is_path_delimiter(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\'' | '`' | '<' | '>' | '[' | ']' | ',' | ';' | '|' | '='
        )
}

/// Replaces absolute filesystem paths by `{path}`; `res://`, `user://` and relative paths stay.
fn redact_paths(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut token_start: Option<usize> = None;
    for (index, c) in text.char_indices() {
        if is_path_delimiter(c) {
            if let Some(start) = token_start.take() {
                push_path_token(&mut out, &text[start..index]);
            }
            out.push(c);
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    if let Some(start) = token_start {
        push_path_token(&mut out, &text[start..]);
    }
    out
}

fn push_path_token(out: &mut String, token: &str) {
    let without_lead = token.trim_start_matches('(');
    let lead = &token[..token.len() - without_lead.len()];
    let core = without_lead.trim_end_matches(['.', ',', ':', ';', '!', '?', ')']);
    let tail = &without_lead[core.len()..];
    out.push_str(lead);
    out.push_str(if is_absolute_path(core) { PATH } else { core });
    out.push_str(tail);
}

fn is_absolute_path(core: &str) -> bool {
    if ["res://", "user://", "uid://"]
        .iter()
        .any(|scheme| core.starts_with(scheme))
    {
        return false;
    }
    let bytes = core.as_bytes();
    let drive_path = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || (bytes[2] == b'/' && bytes.get(3) != Some(&b'/')));
    if drive_path || (core.starts_with("\\\\") && core.len() > 2) {
        return true;
    }
    if core.starts_with('/') {
        let segments: Vec<&str> = core.split('/').filter(|part| !part.is_empty()).collect();
        return segments.len() >= 2
            || segments
                .first()
                .is_some_and(|first| UNIX_ROOTS.contains(first));
    }
    false
}

/// Replaces credential-shaped text by `{redacted}`.
fn redact_credentials(text: &str) -> String {
    fn has_credential_prefix(word: &str) -> bool {
        CREDENTIAL_PREFIXES.iter().any(|prefix| {
            word.strip_prefix(prefix)
                .is_some_and(|rest| rest.chars().count() >= MIN_AFTER_PREFIX)
        })
    }
    fn is_long_mixed_run(run: &str) -> bool {
        run.len() >= LONG_RUN
            && run.chars().any(|c| c.is_ascii_alphabetic())
            && run.chars().any(|c| c.is_ascii_digit())
    }
    let text = replace_runs(
        text,
        |c| c.is_ascii_alphanumeric() || c == '_' || c == '-',
        has_credential_prefix,
    );
    let text = redact_bearer_values(&text);
    replace_runs(&text, |c| c.is_ascii_alphanumeric(), is_long_mixed_run)
}

/// Rewrites every maximal run of characters accepted by `in_run` that `redact` selects.
fn replace_runs(text: &str, in_run: fn(char) -> bool, redact: fn(&str) -> bool) -> String {
    fn push_run(out: &mut String, run: &str, redact: fn(&str) -> bool) {
        out.push_str(if redact(run) { REDACTED } else { run });
    }
    let mut out = String::with_capacity(text.len());
    let mut run_start: Option<usize> = None;
    for (index, c) in text.char_indices() {
        if in_run(c) {
            run_start.get_or_insert(index);
        } else {
            if let Some(start) = run_start.take() {
                push_run(&mut out, &text[start..index], redact);
            }
            out.push(c);
        }
    }
    if let Some(start) = run_start {
        push_run(&mut out, &text[start..], redact);
    }
    out
}

/// Replaces the value that follows the word `Bearer`.
fn redact_bearer_values(text: &str) -> String {
    const WORD: &str = "bearer";
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(found) = lower[cursor..].find(WORD) {
        let word_end = cursor + found + WORD.len();
        let separated_before = text[..cursor + found]
            .chars()
            .next_back()
            .is_none_or(|c| !is_name_char(c));
        let rest = &text[word_end..];
        let spaces = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        let value_len = rest[spaces..]
            .find(char::is_whitespace)
            .unwrap_or(rest.len() - spaces);
        let value = &rest[spaces..spaces + value_len];
        out.push_str(&text[cursor..word_end]);
        if separated_before
            && spaces > 0
            && value.chars().count() >= MIN_AFTER_PREFIX
            && value != REDACTED
        {
            out.push_str(&rest[..spaces]);
            out.push_str(REDACTED);
            cursor = word_end + spaces + value_len;
        } else {
            cursor = word_end;
        }
    }
    out.push_str(&text[cursor..]);
    out
}

/// Keeps text from notifying users (`@name`), cross-linking issues (`#123`), injecting markup
/// (`<`) or making the tracker fetch an image (`![`).
fn neutralize_markdown(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &c) in chars.iter().enumerate() {
        let next = chars.get(index + 1).copied();
        match c {
            '@' if next.is_some_and(|n| n.is_alphanumeric() || n == '_') => {
                out.push('@');
                out.push(ZERO_WIDTH_SPACE);
            }
            '#' if next.is_some_and(|n| n.is_ascii_digit()) => {
                out.push('#');
                out.push(ZERO_WIDTH_SPACE);
            }
            '<' => out.push_str("&lt;"),
            '!' if next == Some('[') => out.push_str("!\\"),
            other => out.push(other),
        }
    }
    out
}

/// What stays the same when the same failure is reported twice: lower case, digit runs as `#`,
/// anything path-like as `<path>`, single spaces, at most 120 characters.
pub fn shape(message: &str) -> String {
    let words: Vec<String> = message
        .split_whitespace()
        .map(|token| {
            if token.contains(['/', '\\']) {
                return "<path>".to_string();
            }
            let mut word = String::new();
            for c in token.to_lowercase().chars() {
                if c.is_ascii_digit() {
                    if !word.ends_with('#') {
                        word.push('#');
                    }
                } else {
                    word.push(c);
                }
            }
            word
        })
        .collect();
    words.join(" ").chars().take(120).collect()
}

/// 64-bit FNV-1a: small, stable across Rust versions and platforms, and good enough to tell
/// reports apart (it is not a security measure).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// One alphanumeric token, so the tracker's search treats it as a single term.
pub fn fingerprint(key: &str) -> String {
    let digits = format!("{:016x}", fnv1a64(key.as_bytes()));
    format!("gmfb{}", &digits[..12])
}

/// Percent-encodes everything except the unreserved characters `A-Z a-z 0-9 - . _ ~`.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        push_percent_encoded(&mut out, c);
    }
    out
}

fn push_percent_encoded(out: &mut String, c: char) {
    if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
        out.push(c);
        return;
    }
    let mut buffer = [0u8; 4];
    for byte in c.encode_utf8(&mut buffer).bytes() {
        out.push_str(&format!("%{byte:02X}"));
    }
}

/// A new-issue link carrying only the title and the body (labels and the like need permissions
/// the reporter may lack). A body that does not fit is cut at a character boundary and ends with
/// `LINK_CUT_MARKER`; when even that does not fit, the link carries the title alone.
pub fn build_link(base: &str, title: &str, body: &str) -> Result<String, String> {
    let head = format!("{base}?title={}", percent_encode(title));
    if head.len() > LINK_BUDGET {
        return Err(format!(
            "the title is too long for a link of {LINK_BUDGET} bytes"
        ));
    }
    let full = format!("{head}&body={}", percent_encode(body));
    if full.len() <= LINK_BUDGET {
        return Ok(full);
    }
    let marker = percent_encode(LINK_CUT_MARKER);
    let fixed = head.len() + "&body=".len() + marker.len();
    if fixed > LINK_BUDGET {
        return Ok(head);
    }
    let mut kept = String::new();
    for c in body.chars() {
        let mut piece = String::new();
        push_percent_encoded(&mut piece, c);
        if fixed + kept.len() + piece.len() > LINK_BUDGET {
            break;
        }
        kept.push_str(&piece);
    }
    Ok(format!("{head}&body={kept}{marker}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn context() -> ScrubContext {
        ScrubContext::default()
    }

    #[test]
    fn control_and_bidirectional_override_characters_are_removed() {
        let text = "a\u{0}b\u{7}c\r\nd\te\u{202E}f\u{2066}g\u{200B}h";
        assert_eq!(scrub(text, &context()), "abc\nd\tefg\u{200B}h");
    }

    #[test]
    fn the_exact_secret_is_replaced_wherever_it_appears() {
        let ctx = ScrubContext {
            secret: Some("s3cr3t-value".into()),
            ..ScrubContext::default()
        };
        assert_eq!(
            scrub("token s3cr3t-value and again s3cr3t-value.", &ctx),
            "token {redacted} and again {redacted}."
        );
    }

    #[test]
    fn an_empty_secret_changes_nothing() {
        let ctx = ScrubContext {
            secret: Some(String::new()),
            ..ScrubContext::default()
        };
        assert_eq!(scrub("plain text", &ctx), "plain text");
    }

    #[test]
    fn the_project_directory_becomes_a_placeholder_in_both_separator_styles() {
        let ctx = ScrubContext {
            project_dir: Some(PathBuf::from(r"C:\Users\Alice\game")),
            ..ScrubContext::default()
        };
        assert_eq!(
            scrub(r"error in C:\Users\Alice\game\scripts\a.gd", &ctx),
            r"error in {project}\scripts\a.gd"
        );
        assert_eq!(
            scrub("error in C:/Users/Alice/game/scripts/a.gd", &ctx),
            "error in {project}/scripts/a.gd"
        );
    }

    #[test]
    fn a_unix_project_directory_with_a_trailing_slash_is_replaced() {
        let ctx = ScrubContext {
            project_dir: Some(PathBuf::from("/home/alice/game/")),
            ..ScrubContext::default()
        };
        assert_eq!(
            scrub("/home/alice/game/scripts/a.gd", &ctx),
            "{project}/scripts/a.gd"
        );
    }

    #[test]
    fn a_directory_match_does_not_continue_into_a_longer_name() {
        assert_eq!(
            replace_directory(
                "/home/alice/game2/x",
                "/home/alice/game",
                "{project}",
                false
            ),
            "/home/alice/game2/x"
        );
        assert_eq!(
            replace_directory(
                "see /home/alice/game, ok",
                "/home/alice/game",
                "{project}",
                false
            ),
            "see {project}, ok"
        );
    }

    #[test]
    fn directory_matching_can_ignore_case() {
        assert_eq!(
            replace_directory(r"c:\users\alice\x", r"C:\Users\Alice", "~", true),
            r"~\x"
        );
        assert_eq!(
            replace_directory(r"c:\users\alice\x", r"C:\Users\Alice", "~", false),
            r"c:\users\alice\x"
        );
    }

    #[test]
    fn the_home_directory_becomes_a_tilde_after_the_project_directory() {
        let ctx = ScrubContext {
            project_dir: Some(PathBuf::from("/home/alice/game")),
            home_dir: Some(PathBuf::from("/home/alice")),
            ..ScrubContext::default()
        };
        assert_eq!(
            scrub("/home/alice/game/a.gd and /home/alice/.config/b", &ctx),
            "{project}/a.gd and ~/.config/b"
        );
    }

    #[test]
    fn without_a_known_project_a_path_under_home_is_hidden_completely() {
        let ctx = ScrubContext {
            home_dir: Some(PathBuf::from("/home/alice")),
            ..ScrubContext::default()
        };
        assert_eq!(
            scrub("opened /home/alice/MyGame/scripts/a.gd", &ctx),
            "opened {path}"
        );
    }

    #[test]
    fn a_root_directory_as_home_is_ignored_instead_of_eating_every_slash() {
        let ctx = ScrubContext {
            home_dir: Some(PathBuf::from("/")),
            ..ScrubContext::default()
        };
        assert_eq!(scrub("res://a/b and x/y", &ctx), "res://a/b and x/y");
    }

    #[test]
    fn absolute_paths_become_placeholders() {
        assert_eq!(
            redact_paths(r"failed to open D:\Games\Foo\bar.gd."),
            "failed to open {path}."
        );
        assert_eq!(
            redact_paths(r"read \\server\share\x.txt now"),
            "read {path} now"
        );
        assert_eq!(
            redact_paths("/usr/local/bin/godot not found"),
            "{path} not found"
        );
        assert_eq!(redact_paths("cwd is /tmp"), "cwd is {path}");
        assert_eq!(redact_paths("(see /home/bob/x.gd)"), "(see {path})");
        assert_eq!(redact_paths("--path=/srv/data/game"), "--path={path}");
        assert_eq!(
            redact_paths(r#"{"file":"/etc/hosts/x"}"#),
            r#"{"file":"{path}"}"#
        );
    }

    #[test]
    fn project_relative_and_non_path_text_is_left_alone() {
        for text in [
            "res://scenes/Main.tscn",
            "user://save/slot1.dat",
            "uid://b3x4k2",
            "https://example.com/a/b/c",
            "Player/Sprite2D/Camera",
            "1/2 of /api",
            "{project}/scripts/a.gd",
            "~/.config/godot",
        ] {
            assert_eq!(redact_paths(text), text, "{text}");
        }
    }

    #[test]
    fn credentials_are_replaced() {
        for secret in [
            "ghp_abcdefghijklmnop1234",
            "gho_abcdefghijklmnop1234",
            "ghs_abcdefghijklmnop1234",
            "github_pat_11ABCDEFG0abcdefghijkl_mnopqrstuv",
            "sk-abcdefgh12345678",
            "xoxb-1234567890-abcdef",
            "xoxp-1234567890-abcdef",
            "AKIAIOSFODNN7EXAMPLE",
        ] {
            assert_eq!(
                redact_credentials(&format!("key {secret} end")),
                "key {redacted} end",
                "{secret}"
            );
        }
        assert_eq!(
            redact_credentials("Authorization: Bearer abc.def-ghi_jkl and more"),
            "Authorization: Bearer {redacted} and more"
        );
        assert_eq!(
            redact_credentials("hash 0123456789abcdef0123456789abcdef end"),
            "hash {redacted} end"
        );
    }

    #[test]
    fn ordinary_identifiers_and_ids_survive_the_credential_rule() {
        for text in [
            "abcdefghijklmnopqrstuvwxyzabcdef",
            "0123456789012345678901234567890",
            "123e4567-e89b-12d3-a456-426614174000",
            "res://levels/very_long_level_name_with_digits_1234567890_abcdefghijklmnop.tscn",
            "task-queue and mask-abcdefgh12345678",
            "sk-short",
            "godot_set_node_properties",
            "Bearer",
        ] {
            assert_eq!(redact_credentials(text), text, "{text}");
        }
    }

    #[test]
    fn markdown_that_notifies_or_embeds_is_neutralized() {
        assert_eq!(
            neutralize_markdown("hi @alice and @everyone"),
            "hi @\u{200B}alice and @\u{200B}everyone"
        );
        assert_eq!(neutralize_markdown("see #123"), "see #\u{200B}123");
        assert_eq!(neutralize_markdown("# title and #tag"), "# title and #tag");
        assert_eq!(
            neutralize_markdown("<script>alert(1)</script>"),
            "&lt;script>alert(1)&lt;/script>"
        );
        assert_eq!(
            neutralize_markdown("![x](http://e/p.png)"),
            r"!\[x](http://e/p.png)"
        );
        assert_eq!(neutralize_markdown("a@b.com"), "a@\u{200B}b.com");
    }

    #[test]
    fn scrub_applies_every_step_to_one_text() {
        let ctx = ScrubContext {
            secret: Some("topsecretvalue".into()),
            project_dir: Some(PathBuf::from("/home/alice/game")),
            home_dir: Some(PathBuf::from("/home/alice")),
        };
        let text = "ping @bob see #12 at /home/alice/game/a.gd and /opt/x/y with topsecretvalue, \
                    ghp_abcdefghijklmnop1234, C:\\Tools\\godot.exe, \
                    0123456789abcdef0123456789abcdef <b>";
        assert_eq!(
            scrub(text, &ctx),
            "ping @\u{200B}bob see #\u{200B}12 at {project}/a.gd and {path} with {redacted}, \
             {redacted}, {path}, {redacted} &lt;b>"
        );
    }

    #[test]
    fn scrubbing_twice_changes_nothing_more() {
        let ctx = ScrubContext {
            secret: Some("topsecretvalue".into()),
            project_dir: Some(PathBuf::from("/home/alice/game")),
            home_dir: Some(PathBuf::from("/home/alice")),
        };
        let text = "@bob #12 <b> ![i](u) /home/alice/game/a.gd /opt/x/y topsecretvalue \
                    ghp_abcdefghijklmnop1234 C:\\Tools\\godot.exe 0123456789abcdef0123456789abcdef";
        let once = scrub(text, &ctx);
        assert_eq!(scrub(&once, &ctx), once);
        assert!(!once.contains("alice"));
        assert!(!once.contains("topsecretvalue"));
    }

    #[test]
    fn newlines_and_tabs_survive() {
        assert_eq!(
            scrub("line one\n\tline two", &context()),
            "line one\n\tline two"
        );
    }

    #[test]
    fn fnv1a_matches_the_published_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn a_fingerprint_is_one_stable_alphanumeric_token() {
        let first = fingerprint("bug|godot_add_node");
        assert_eq!(first, fingerprint("bug|godot_add_node"));
        assert_ne!(first, fingerprint("bug|godot_remove_node"));
        assert!(first.starts_with("gmfb"));
        assert_eq!(first.len(), 16);
        assert!(
            first
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn a_message_shape_ignores_numbers_paths_case_and_spacing() {
        assert_eq!(
            shape("Failed  at /home/u/a.gd line 12, col 7"),
            "failed at <path> line #, col #"
        );
        assert_eq!(
            shape("Failed at res://x/b.gd line 99, col 3"),
            "failed at <path> line #, col #"
        );
        assert_eq!(shape(&"x".repeat(500)).len(), 120);
    }

    #[test]
    fn percent_encoding_keeps_only_unreserved_characters() {
        assert_eq!(percent_encode("AZaz09-._~"), "AZaz09-._~");
        assert_eq!(percent_encode(" /?#&=+%\n"), "%20%2F%3F%23%26%3D%2B%25%0A");
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    const BASE: &str = "https://github.com/o/r/issues/new";

    #[test]
    fn a_short_report_becomes_a_complete_link() {
        assert_eq!(
            build_link(BASE, "A b", "Body").expect("link"),
            "https://github.com/o/r/issues/new?title=A%20b&body=Body"
        );
    }

    #[test]
    fn a_long_body_is_cut_to_the_budget_and_says_so() {
        let link = build_link(BASE, "T", &"a".repeat(20_000)).expect("link");
        assert!(link.len() <= LINK_BUDGET, "{}", link.len());
        assert!(link.ends_with(&percent_encode(LINK_CUT_MARKER)));
        assert!(link.contains("&body=aaaa"));
    }

    #[test]
    fn a_cut_never_splits_a_multibyte_character() {
        let link = build_link(BASE, "T", &"é".repeat(10_000)).expect("link");
        assert!(link.len() <= LINK_BUDGET);
        let body = link.split("&body=").nth(1).expect("body part");
        let decoded = decode_percent(body);
        assert!(String::from_utf8(decoded).is_ok());
    }

    #[test]
    fn a_title_that_leaves_no_room_for_the_marker_gives_a_title_only_link() {
        let title = "x".repeat(LINK_BUDGET - BASE.len() - "?title=".len() - 10);
        let link = build_link(BASE, &title, &"b".repeat(1_000)).expect("link");
        assert!(link.len() <= LINK_BUDGET);
        assert!(!link.contains("body="));
    }

    #[test]
    fn a_title_that_cannot_fit_gives_no_link() {
        let error = build_link(BASE, &"x".repeat(7_000), "b").expect_err("no link");
        assert!(error.contains("too long"), "{error}");
    }

    fn decode_percent(text: &str) -> Vec<u8> {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("hex digits");
                out.push(u8::from_str_radix(hex, 16).expect("hex digits"));
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        out
    }
}
