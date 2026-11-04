//! Container logs: the one command that is built from input, and a parser for its output.
//!
//! The command contains exactly two request-controlled values, and both are validated so they
//! cannot change what runs: the container id must be 12 to 64 lowercase hexadecimal characters
//! (so it cannot start with `-` or contain a space, quote or any shell character), and the line
//! count must be a number from 1 to [`MAX_TAIL`]. Everything else in the command is constant.
//!
//! Log text is untrusted output of arbitrary programs: escape sequences and control characters
//! are removed, lines are length-limited, and errors never echo it.

use super::{DockerOutcome, MSG_NOT_INSTALLED, known_problem, run_once};
use crate::ssh::client::TestParams;
use serde::Serialize;

pub const MIN_TAIL: u32 = 1;
pub const MAX_TAIL: u32 = 1000;
/// At most this many lines are returned; the newest are kept.
pub const MAX_LINES: usize = 1000;
/// Longer lines are cut and end with an ellipsis.
pub const MAX_LINE_CHARS: usize = 2000;

/// Everything in the command that precedes the request-controlled values. The test server uses it
/// to recognize the request.
pub const COMMAND_PREFIX: &str =
    "sh -c 'command -v docker >/dev/null 2>&1 || exit 127; docker logs --timestamps --tail ";

const MSG_NOT_FOUND: &str =
    "That container no longer exists. Reload the container list and choose another.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRequest {
    pub field: &'static str,
    pub message: &'static str,
}

/// A validated request for a container's recent logs. The only way to get one is [`new`], so a
/// value of this type always holds a safe command.
///
/// [`new`]: LogsRequest::new
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogsRequest {
    command: String,
}

fn valid_container_id(id: &str) -> bool {
    (12..=64).contains(&id.len()) && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl LogsRequest {
    pub fn new(container_id: &str, tail: u32) -> Result<Self, InvalidRequest> {
        if !valid_container_id(container_id) {
            return Err(InvalidRequest {
                field: "containerId",
                message: "Choose a container from the list.",
            });
        }
        if !(MIN_TAIL..=MAX_TAIL).contains(&tail) {
            return Err(InvalidRequest {
                field: "tail",
                message: "Choose between 1 and 1000 lines.",
            });
        }
        // `2>&1` because containers write errors to stderr; both streams are the log.
        Ok(Self {
            command: format!("{COMMAND_PREFIX}{tail} {container_id} 2>&1'"),
        })
    }

    pub fn command(&self) -> &str {
        &self.command
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    /// Docker's RFC 3339 timestamp, when the line had one in the expected shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub lines: Vec<LogLine>,
    /// Older lines were dropped to stay within [`MAX_LINES`].
    pub truncated: bool,
}

/// Removes terminal escape sequences (colors, cursor movement, window titles) and the lone
/// escape characters left behind by cut-off ones.
fn strip_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: ESC [ parameters... final byte (0x40-0x7e)
            Some('[') => {
                chars.next();
                for n in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&n) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... terminated by BEL or ESC \
            Some(']') => {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '\u{7}' {
                        break;
                    }
                    if n == '\u{1b}' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            // Any other two-character escape.
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// Escapes removed, other control characters removed (a tab becomes a space), length limited.
fn clean_message(s: &str) -> String {
    let stripped = strip_escapes(s);
    let mut out = String::new();
    let mut count = 0;
    for c in stripped.chars() {
        let c = if c == '\t' { ' ' } else { c };
        if c.is_control() {
            continue;
        }
        if count == MAX_LINE_CHARS {
            out.push('…');
            break;
        }
        out.push(c);
        count += 1;
    }
    out
}

/// Whether `token` looks like the RFC 3339 timestamp `docker logs --timestamps` puts first.
fn looks_like_timestamp(token: &str) -> bool {
    let b = token.as_bytes();
    (20..=40).contains(&b.len())
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && token.contains('T')
        && token
            .bytes()
            .all(|c| c.is_ascii_digit() || matches!(c, b'T' | b'Z' | b':' | b'.' | b'+' | b'-'))
}

fn parse_line(raw: &str) -> Option<LogLine> {
    let raw = raw.trim_end_matches('\r');
    let (time, rest) = match raw.split_once(' ') {
        Some((first, rest)) if looks_like_timestamp(first) => (Some(first.to_string()), rest),
        None if looks_like_timestamp(raw) => (Some(raw.to_string()), ""),
        _ => (None, raw),
    };
    let message = clean_message(rest);
    // Blank lines without a timestamp carry nothing.
    if time.is_none() && message.is_empty() {
        return None;
    }
    Some(LogLine { time, message })
}

/// Parses the output of a logs request. Any text is valid log output, so this cannot fail.
pub fn parse_logs(output: &str) -> Parsed {
    let mut lines: Vec<LogLine> = output.split('\n').filter_map(parse_line).collect();
    let truncated = lines.len() > MAX_LINES;
    if truncated {
        lines.drain(..lines.len() - MAX_LINES);
    }
    Parsed { lines, truncated }
}

/// What a logs request produced. Connection and login problems are returned as errors instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsResult {
    pub outcome: DockerOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub lines: Vec<LogLine>,
    pub truncated: bool,
    /// Unix epoch milliseconds, set by VPSPilot (not the remote clock).
    pub fetched_at: i64,
}

fn failed(outcome: DockerOutcome, message: &str, now: i64) -> LogsResult {
    LogsResult {
        outcome,
        message: Some(message.to_string()),
        lines: Vec::new(),
        truncated: false,
        fetched_at: now,
    }
}

/// With `2>&1` the output of a failed command is Docker's error text, so it is only used to
/// recognize known problems and is never shown.
fn classify(exit: u32, output: &str, now: i64) -> LogsResult {
    match exit {
        0 => {
            let parsed = parse_logs(output);
            LogsResult {
                outcome: DockerOutcome::Ok,
                message: None,
                lines: parsed.lines,
                truncated: parsed.truncated,
                fetched_at: now,
            }
        }
        127 => failed(DockerOutcome::NotInstalled, MSG_NOT_INSTALLED, now),
        code => {
            if output.to_ascii_lowercase().contains("no such container") {
                return failed(DockerOutcome::ContainerNotFound, MSG_NOT_FOUND, now);
            }
            match known_problem(output) {
                Some((outcome, message)) => failed(outcome, message, now),
                None => failed(
                    DockerOutcome::Error,
                    &format!("Docker reported an error (exit status {code})."),
                    now,
                ),
            }
        }
    }
}

/// Fetches a container's recent logs. Returns `Err` with a user-safe message for anything that
/// stops it reaching Docker at all (untrusted host key, login failure, network problem).
pub async fn fetch(params: TestParams, request: &LogsRequest) -> Result<LogsResult, String> {
    let out = run_once(&params, request.command()).await?;
    // Merged output can only be too large, never partial: `run_command` rejects it above the cap.
    Ok(classify(
        out.exit,
        &String::from_utf8_lossy(&out.stdout),
        crate::servers::repo::now_ms(),
    ))
}

#[cfg(test)]
pub const SAMPLE_LOGS: &str = concat!(
    "2026-11-04T10:00:00.123456789Z server listening on :8080\n",
    "2026-11-04T10:00:01.000000000Z \u{1b}[31mERROR\u{1b}[0m failed to reach db\n",
    "2026-11-04T10:00:02.500000000Z request ok\n",
);

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "a1b2c3d4e5f6";

    #[test]
    fn builds_the_expected_command() {
        let r = LogsRequest::new(ID, 200).unwrap();
        assert_eq!(
            r.command(),
            "sh -c 'command -v docker >/dev/null 2>&1 || exit 127; docker logs --timestamps --tail 200 a1b2c3d4e5f6 2>&1'"
        );
    }

    #[test]
    fn the_built_command_is_safe_to_hand_to_a_shell() {
        let longest = "f".repeat(64);
        for (id, tail) in [(ID, 1), (ID, 1000), (longest.as_str(), 500)] {
            let c = LogsRequest::new(id, tail).unwrap();
            let cmd = c.command();
            assert!(cmd.starts_with(COMMAND_PREFIX) && cmd.ends_with('\''));
            // Only the wrapper's own two quotes: nothing request-controlled can add one.
            assert_eq!(cmd.matches('\'').count(), 2, "{cmd}");
            let script = &cmd["sh -c '".len()..cmd.len() - 1];
            for bad in ['$', '`', ';', '|', '<', '"', '\\', '\n', '(', ')', '{', '}'] {
                // The constant prefix has `;` and `||`; check only the request-controlled tail.
                let tail_part = &script[script.find("--tail").unwrap()..];
                assert!(!tail_part.contains(bad), "{bad:?} in {tail_part}");
            }
            assert_eq!(cmd.matches("docker logs").count(), 1);
        }
    }

    #[test]
    fn rejects_anything_that_is_not_a_plain_container_id() {
        let too_long = "a".repeat(65);
        for bad in [
            "",
            "abc",
            "a1b2c3d4e5f",            // 11 characters
            too_long.as_str(),        // too long
            "A1B2C3D4E5F6",           // uppercase
            "a1b2c3d4e5fg",           // not hex
            "-rf000000000",           // option-like
            "a1b2c3d4e5f6; rm -rf /", // injection
            "a1b2c3d4e5f6 && id",
            "a1b2c3d4e5f6'",
            "a1b2c3d4e5f6\n",
            "$(id)aaaaaaaa",
            "`id`aaaaaaaaa",
            "a1b2c3d4 e5f6",
            "web",
            "../../etc/passwd",
            "ａ１ｂ２ｃ３ｄ４ｅ５ｆ６", // full-width look-alikes
        ] {
            let err = LogsRequest::new(bad, 100).unwrap_err();
            assert_eq!(err.field, "containerId", "{bad:?}");
        }
    }

    #[test]
    fn accepts_the_length_boundaries_of_an_id() {
        assert!(LogsRequest::new(&"0".repeat(12), 10).is_ok());
        assert!(LogsRequest::new(&"0".repeat(64), 10).is_ok());
    }

    #[test]
    fn tail_is_bounded() {
        for bad in [0, MAX_TAIL + 1, u32::MAX] {
            assert_eq!(LogsRequest::new(ID, bad).unwrap_err().field, "tail");
        }
        assert!(LogsRequest::new(ID, MIN_TAIL).is_ok());
        assert!(LogsRequest::new(ID, MAX_TAIL).is_ok());
    }

    #[test]
    fn parses_timestamps_and_messages() {
        let p = parse_logs(SAMPLE_LOGS);
        assert!(!p.truncated);
        assert_eq!(p.lines.len(), 3);
        assert_eq!(
            p.lines[0].time.as_deref(),
            Some("2026-11-04T10:00:00.123456789Z")
        );
        assert_eq!(p.lines[0].message, "server listening on :8080");
        assert_eq!(p.lines[2].message, "request ok");
    }

    #[test]
    fn colors_are_stripped() {
        let p = parse_logs(SAMPLE_LOGS);
        assert_eq!(p.lines[1].message, "ERROR failed to reach db");
    }

    #[test]
    fn strips_every_kind_of_escape_sequence() {
        let cases = [
            ("a\u{1b}[1;31mb\u{1b}[0mc", "abc"),
            ("a\u{1b}[2Jb", "ab"),
            ("a\u{1b}[?25lb", "ab"),
            ("a\u{1b}]0;evil title\u{7}b", "ab"),
            (
                "a\u{1b}]8;;http://x\u{1b}\\link\u{1b}]8;;\u{1b}\\b",
                "alinkb",
            ),
            ("a\u{1b}cb", "ab"),
            ("a\u{1b}", "a"),
            ("a\u{1b}[", "a"),
            ("a\u{1b}[31", "a"),
        ];
        for (input, want) in cases {
            assert_eq!(strip_escapes(input), want, "{input:?}");
        }
    }

    #[test]
    fn other_control_characters_are_removed() {
        let p = parse_logs(
            "2026-11-04T10:00:00Z a\u{7}b\u{8}c\u{0}d\r\n2026-11-04T10:00:01Z x\u{9b}y\n",
        );
        assert_eq!(p.lines[0].message, "abcd");
        assert_eq!(p.lines[1].message, "xy");
    }

    #[test]
    fn tabs_become_spaces() {
        let p = parse_logs("2026-11-04T10:00:00Z a\tb\n");
        assert_eq!(p.lines[0].message, "a b");
    }

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        let line = format!("2026-11-04T10:00:00Z {}\n", "x".repeat(MAX_LINE_CHARS * 3));
        let m = &parse_logs(&line).lines[0].message;
        assert_eq!(m.chars().count(), MAX_LINE_CHARS + 1);
        assert!(m.ends_with('…'));

        let exact = format!("2026-11-04T10:00:00Z {}\n", "x".repeat(MAX_LINE_CHARS));
        assert!(!parse_logs(&exact).lines[0].message.ends_with('…'));
    }

    #[test]
    fn lines_without_a_timestamp_are_kept_whole() {
        let p = parse_logs("no timestamp here\nsecond line\n");
        assert_eq!(p.lines[0].time, None);
        assert_eq!(p.lines[0].message, "no timestamp here");
        assert_eq!(p.lines.len(), 2);
    }

    #[test]
    fn only_a_well_formed_timestamp_is_split_off() {
        for first in [
            "hello",
            "2026-11-04",
            "12345678901234567890",
            "2026-11-04T10:00:00Zjunk",
        ] {
            let p = parse_logs(&format!("{first} rest\n"));
            assert_eq!(p.lines[0].time, None, "{first}");
            assert_eq!(p.lines[0].message, format!("{first} rest"));
        }
        // Offsets and short fractions are fine.
        let p = parse_logs("2026-11-04T10:00:00+02:00 hi\n2026-11-04T10:00:00.5Z hi\n");
        assert!(p.lines[0].time.is_some() && p.lines[1].time.is_some());
    }

    #[test]
    fn a_timestamp_with_an_empty_message_is_kept() {
        let p = parse_logs("2026-11-04T10:00:00Z \n2026-11-04T10:00:01Z\n");
        assert_eq!(p.lines.len(), 2);
        assert_eq!(p.lines[0].message, "");
        assert_eq!(p.lines[1].message, "");
    }

    #[test]
    fn blank_lines_without_a_timestamp_are_dropped() {
        // Spaces count as content, so only truly empty lines vanish.
        let p = parse_logs("\n\nreal\n\n");
        assert_eq!(p.lines.len(), 1);
        assert_eq!(p.lines[0].message, "real");
    }

    #[test]
    fn windows_line_endings_are_handled() {
        let p = parse_logs("2026-11-04T10:00:00Z a\r\n2026-11-04T10:00:01Z b\r\n");
        assert_eq!(p.lines[0].message, "a");
        assert_eq!(p.lines[1].message, "b");
    }

    #[test]
    fn empty_output_is_no_lines() {
        for out in ["", "\n", "\n\n"] {
            let p = parse_logs(out);
            assert!(p.lines.is_empty() && !p.truncated);
        }
    }

    #[test]
    fn invalid_utf8_is_replaced_not_rejected() {
        let bytes = b"2026-11-04T10:00:00Z bad \xff\xfe bytes\n";
        let p = parse_logs(&String::from_utf8_lossy(bytes));
        assert_eq!(p.lines.len(), 1);
        assert!(p.lines[0].message.starts_with("bad "));
    }

    #[test]
    fn keeps_the_newest_lines_when_there_are_too_many() {
        let out: String = (0..MAX_LINES + 5)
            .map(|i| format!("2026-11-04T10:00:00Z line {i}\n"))
            .collect();
        let p = parse_logs(&out);
        assert!(p.truncated);
        assert_eq!(p.lines.len(), MAX_LINES);
        assert_eq!(p.lines[0].message, "line 5");
        assert_eq!(
            p.lines.last().unwrap().message,
            format!("line {}", MAX_LINES + 4)
        );
    }

    #[test]
    fn classify_recognizes_known_problems_without_echoing_text() {
        let c = |exit, text| classify(exit, text, 1);

        assert_eq!(c(0, SAMPLE_LOGS).outcome, DockerOutcome::Ok);
        assert_eq!(c(127, "").outcome, DockerOutcome::NotInstalled);

        let nf = c(
            1,
            "Error response from daemon: No such container: abc SECRET",
        );
        assert_eq!(nf.outcome, DockerOutcome::ContainerNotFound);
        assert!(!nf.message.unwrap().contains("SECRET"));

        let denied = c(1, "permission denied while trying to connect SECRET");
        assert_eq!(denied.outcome, DockerOutcome::PermissionDenied);
        assert!(!denied.message.unwrap().contains("SECRET"));

        let down = c(
            1,
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock SECRET",
        );
        assert_eq!(down.outcome, DockerOutcome::DaemonUnavailable);
        assert!(!down.message.unwrap().contains("SECRET"));

        let other = c(3, "something odd SECRET");
        assert_eq!(other.outcome, DockerOutcome::Error);
        let m = other.message.unwrap();
        assert!(m.contains("exit status 3") && !m.contains("SECRET"));
        assert!(other.lines.is_empty());
    }
}
