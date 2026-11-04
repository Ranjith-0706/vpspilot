//! The one command VPSPilot runs to list Docker containers, and a strict parser for its output.
//!
//! Output comes from a remote machine and is therefore untrusted: parsing is bounded, every
//! string is sanitized and length-limited, and nothing is ever interpreted as a command or path.
//! Parse errors never include the remote text.

use serde::{Deserialize, Serialize};

/// The *only* command the Docker feature ever runs. It is a constant: no part of it comes from the
/// frontend or from stored data, and it takes no container name or id, so there is nothing to
/// inject into. It is read-only (`docker ps` only lists) and never uses `sudo`: the account must
/// already be allowed to use Docker.
///
/// `{{json .}}` prints one JSON object per container, which survives spaces, commas and arrows in
/// image, status and port text that a delimiter-based format would not. Exit status 127 means the
/// `docker` command does not exist.
///
/// The script must not contain a single quote, `$` or a backtick (checked by a test).
pub const COMMAND: &str = concat!(
    "sh -c '",
    "command -v docker >/dev/null 2>&1 || exit 127; ",
    "docker ps -a --format \"{{json .}}\"",
    "'"
);

/// Remote output larger than this is rejected.
pub const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
/// Only this much error text is kept, and only to recognize known problems. It is never shown.
pub const MAX_STDERR_BYTES: usize = 8 * 1024;
/// More containers than this are dropped and the result is marked truncated.
pub const MAX_CONTAINERS: usize = 500;

const MAX_ID: usize = 64;
const MAX_NAME: usize = 128;
const MAX_IMAGE: usize = 256;
const MAX_STATUS: usize = 200;
const MAX_PORTS: usize = 500;
const MAX_CREATED: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerState {
    Running,
    Exited,
    Restarting,
    Paused,
    Created,
    Dead,
    Removing,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Healthy,
    Unhealthy,
    Starting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Container {
    /// The short id Docker prints (first 12 characters).
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: ContainerState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
    /// Docker's own human text, e.g. "Up 3 hours (healthy)".
    pub status: String,
    pub ports: String,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub containers: Vec<Container>,
    /// True when there were more than [`MAX_CONTAINERS`].
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn unreadable() -> ParseError {
    ParseError("Docker returned output VPSPilot could not read.".into())
}

/// The fields we use from `docker ps --format '{{json .}}'`. Everything else (labels, mounts,
/// networks, ...) is ignored, and every field is optional because versions differ.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct Raw {
    #[serde(rename = "ID")]
    id: Option<String>,
    names: Option<String>,
    image: Option<String>,
    state: Option<String>,
    status: Option<String>,
    ports: Option<String>,
    created_at: Option<String>,
}

/// Drops control characters (so remote text cannot move a cursor or fake a line) and limits the
/// length in characters.
fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect()
}

fn state_from_word(word: &str) -> Option<ContainerState> {
    match word {
        "running" => Some(ContainerState::Running),
        "exited" => Some(ContainerState::Exited),
        "restarting" => Some(ContainerState::Restarting),
        "paused" => Some(ContainerState::Paused),
        "created" => Some(ContainerState::Created),
        "dead" => Some(ContainerState::Dead),
        "removing" => Some(ContainerState::Removing),
        _ => None,
    }
}

/// Old Docker versions have no `State` field, so fall back to the start of the status text.
fn state_from_status(status: &str) -> ContainerState {
    let s = status.to_ascii_lowercase();
    if s.starts_with("up") {
        if s.contains("(paused)") {
            ContainerState::Paused
        } else {
            ContainerState::Running
        }
    } else if s.starts_with("exited") {
        ContainerState::Exited
    } else if s.starts_with("restarting") {
        ContainerState::Restarting
    } else if s.starts_with("created") {
        ContainerState::Created
    } else if s.starts_with("dead") {
        ContainerState::Dead
    } else {
        ContainerState::Unknown
    }
}

fn health_from_status(status: &str) -> Option<Health> {
    let s = status.to_ascii_lowercase();
    if s.contains("(unhealthy)") {
        Some(Health::Unhealthy)
    } else if s.contains("(healthy)") {
        Some(Health::Healthy)
    } else if s.contains("(health: starting)") {
        Some(Health::Starting)
    } else {
        None
    }
}

fn convert(raw: Raw) -> Result<Container, ParseError> {
    let id: String = raw
        .id
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(MAX_ID)
        .collect();
    if id.is_empty() {
        return Err(unreadable());
    }

    let status = clean(raw.status.as_deref().unwrap_or(""), MAX_STATUS);
    let state = raw
        .state
        .as_deref()
        .and_then(|s| state_from_word(&s.trim().to_ascii_lowercase()))
        .unwrap_or_else(|| state_from_status(&status));

    // `Names` is a comma-separated list; the first is the container's own name.
    let first_name = raw
        .names
        .as_deref()
        .and_then(|n| n.split(',').next())
        .unwrap_or("")
        .trim()
        .trim_start_matches('/');
    let mut name = clean(first_name, MAX_NAME);
    if name.is_empty() {
        name = id.chars().take(12).collect();
    }

    Ok(Container {
        id: id.chars().take(12).collect(),
        name,
        image: clean(raw.image.as_deref().unwrap_or(""), MAX_IMAGE),
        state,
        health: health_from_status(&status),
        status,
        ports: clean(raw.ports.as_deref().unwrap_or(""), MAX_PORTS),
        created: clean(raw.created_at.as_deref().unwrap_or(""), MAX_CREATED),
    })
}

/// Parses the output of [`COMMAND`]: one JSON object per line, blank lines ignored.
pub fn parse_output(output: &str) -> Result<Parsed, ParseError> {
    if output.len() > MAX_OUTPUT_BYTES {
        return Err(ParseError("Docker returned too much output.".into()));
    }
    let mut containers = Vec::new();
    let mut truncated = false;
    for line in output.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if containers.len() >= MAX_CONTAINERS {
            truncated = true;
            break;
        }
        let raw: Raw = serde_json::from_str(line).map_err(|_| unreadable())?;
        containers.push(convert(raw)?);
    }
    Ok(Parsed {
        containers,
        truncated,
    })
}

/// Output of the Docker CLI with typical values, for tests.
#[cfg(test)]
pub const SAMPLE_OUTPUT: &str = concat!(
    r#"{"ID":"a1b2c3d4e5f6","Names":"web","Image":"nginx:1.27","State":"running","Status":"Up 3 hours (healthy)","Ports":"0.0.0.0:80->80/tcp, :::80->80/tcp","CreatedAt":"2026-11-01 10:00:00 +0000 UTC","Labels":"a=b,c=d"}"#,
    "\n",
    r#"{"ID":"b2c3d4e5f6a1","Names":"db","Image":"postgres:16","State":"running","Status":"Up 2 days (unhealthy)","Ports":"5432/tcp","CreatedAt":"2026-10-30 08:00:00 +0000 UTC"}"#,
    "\n",
    r#"{"ID":"c3d4e5f6a1b2","Names":"old-job","Image":"alpine","State":"exited","Status":"Exited (0) 5 days ago","Ports":"","CreatedAt":"2026-10-20 08:00:00 +0000 UTC"}"#,
    "\n",
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_is_read_only_and_cannot_be_injected_into() {
        assert!(COMMAND.starts_with("sh -c '") && COMMAND.ends_with('\''));
        let script = &COMMAND["sh -c '".len()..COMMAND.len() - 1];
        for bad in ['\'', '$', '`'] {
            assert!(!script.contains(bad), "script contains {bad:?}");
        }
        for word in [
            "sudo", " rm ", "kill", "stop", "start", "restart", "exec", "run ", "pull", "prune",
            "--force", "-f ",
        ] {
            assert!(!COMMAND.contains(word), "command mentions {word:?}");
        }
        assert!(COMMAND.contains("docker ps -a --format"));
        assert_eq!(COMMAND.matches("docker ps").count(), 1);
    }

    #[test]
    fn parses_realistic_output() {
        let p = parse_output(SAMPLE_OUTPUT).unwrap();
        assert!(!p.truncated);
        assert_eq!(p.containers.len(), 3);

        let web = &p.containers[0];
        assert_eq!(web.id, "a1b2c3d4e5f6");
        assert_eq!(web.name, "web");
        assert_eq!(web.image, "nginx:1.27");
        assert_eq!(web.state, ContainerState::Running);
        assert_eq!(web.health, Some(Health::Healthy));
        assert_eq!(web.ports, "0.0.0.0:80->80/tcp, :::80->80/tcp");

        assert_eq!(p.containers[1].health, Some(Health::Unhealthy));
        assert_eq!(p.containers[2].state, ContainerState::Exited);
        assert_eq!(p.containers[2].health, None);
    }

    #[test]
    fn empty_output_means_no_containers() {
        for out in ["", "\n", "  \n\n  "] {
            let p = parse_output(out).unwrap();
            assert!(p.containers.is_empty() && !p.truncated);
        }
    }

    #[test]
    fn name_loses_slash_and_extra_names() {
        let p = parse_output(r#"{"ID":"abc123","Names":"/web,/other/alias","State":"running"}"#)
            .unwrap();
        assert_eq!(p.containers[0].name, "web");
    }

    #[test]
    fn missing_name_falls_back_to_the_id() {
        let p = parse_output(r#"{"ID":"abcdef1234567890","State":"exited"}"#).unwrap();
        assert_eq!(p.containers[0].name, "abcdef123456");
        assert_eq!(p.containers[0].id, "abcdef123456");
    }

    #[test]
    fn state_comes_from_status_when_the_field_is_missing() {
        let state = |status: &str| {
            let line = format!(r#"{{"ID":"a","Status":"{status}"}}"#);
            parse_output(&line).unwrap().containers[0].state
        };
        assert_eq!(state("Up 5 minutes"), ContainerState::Running);
        assert_eq!(state("Up 5 minutes (Paused)"), ContainerState::Paused);
        assert_eq!(state("Exited (1) 2 hours ago"), ContainerState::Exited);
        assert_eq!(
            state("Restarting (1) 3 seconds ago"),
            ContainerState::Restarting
        );
        assert_eq!(state("Created"), ContainerState::Created);
        assert_eq!(state("Dead"), ContainerState::Dead);
        assert_eq!(state("something new"), ContainerState::Unknown);
    }

    #[test]
    fn unknown_state_word_falls_back_to_status() {
        let p = parse_output(r#"{"ID":"a","State":"weird","Status":"Up 1 minute"}"#).unwrap();
        assert_eq!(p.containers[0].state, ContainerState::Running);
    }

    #[test]
    fn health_variants() {
        let health = |status: &str| {
            let line = format!(r#"{{"ID":"a","State":"running","Status":"{status}"}}"#);
            parse_output(&line).unwrap().containers[0].health
        };
        assert_eq!(health("Up 1 hour (healthy)"), Some(Health::Healthy));
        assert_eq!(health("Up 1 hour (unhealthy)"), Some(Health::Unhealthy));
        assert_eq!(
            health("Up 1 second (health: starting)"),
            Some(Health::Starting)
        );
        assert_eq!(health("Up 1 hour"), None);
    }

    #[test]
    fn control_characters_are_removed_and_text_is_truncated() {
        let line = serde_json::json!({
            "ID": "abc",
            "Names": "we\u{1b}[31mb\r\nx",
            "Image": "i".repeat(10_000),
            "Status": "Up\u{7}",
            "Ports": "p".repeat(10_000),
        })
        .to_string();
        let c = &parse_output(&line).unwrap().containers[0];
        assert_eq!(c.name, "we[31mbx");
        assert_eq!(c.image.chars().count(), MAX_IMAGE);
        assert_eq!(c.ports.chars().count(), MAX_PORTS);
        assert_eq!(c.status, "Up");
        assert!(
            ![&c.name, &c.image, &c.status, &c.ports, &c.created]
                .iter()
                .any(|s| s.chars().any(char::is_control))
        );
    }

    #[test]
    fn id_keeps_only_alphanumerics() {
        let p = parse_output(r#"{"ID":"a1-b2;c3 rm -rf","Names":"x"}"#).unwrap();
        assert_eq!(p.containers[0].id, "a1b2c3rmrf");
    }

    #[test]
    fn rejects_unreadable_lines_without_echoing_them() {
        for bad in [
            "not json at all SECRET-TEXT",
            r#"{"ID":"abc""#,
            r#"{"Names":"no id"}"#,
            r#"{"ID":""}"#,
            r#"{"ID":"!!!"}"#,
            r#"{"ID":123}"#,
            "[]",
        ] {
            let err = parse_output(bad).unwrap_err();
            assert!(!err.0.contains("SECRET"), "{err}");
            assert!(!err.0.contains("no id"), "{err}");
        }
    }

    #[test]
    fn one_bad_line_rejects_the_whole_output() {
        let out = format!("{SAMPLE_OUTPUT}garbage\n");
        assert!(parse_output(&out).is_err());
    }

    #[test]
    fn ignores_fields_it_does_not_use() {
        let p = parse_output(
            r#"{"ID":"abc","Names":"x","Labels":{"a":"b"},"Mounts":["m"],"Size":"0B","Extra":null}"#,
        )
        .unwrap();
        assert_eq!(p.containers.len(), 1);
    }

    #[test]
    fn too_many_containers_are_cut_and_flagged() {
        let line = r#"{"ID":"abc","Names":"x","State":"running"}"#;
        let out = std::iter::repeat_n(line, MAX_CONTAINERS + 50)
            .collect::<Vec<_>>()
            .join("\n");
        let p = parse_output(&out).unwrap();
        assert_eq!(p.containers.len(), MAX_CONTAINERS);
        assert!(p.truncated);

        let exact = std::iter::repeat_n(line, MAX_CONTAINERS)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!parse_output(&exact).unwrap().truncated);
    }

    #[test]
    fn oversized_output_is_rejected() {
        let out = "x".repeat(MAX_OUTPUT_BYTES + 1);
        assert!(parse_output(&out).is_err());
    }
}
