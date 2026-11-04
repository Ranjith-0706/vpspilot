//! The one command VPSPilot runs for monitoring, and a strict parser for its output.
//!
//! Output comes from a remote machine and is therefore untrusted: parsing is bounded, numeric,
//! and never interprets any text as a command or path.

use serde::Serialize;

/// The *only* command the monitor ever runs. It is a constant: no part of it comes from the
/// frontend or from stored data. It is read-only (reads /proc and `df /`), needs no privileges,
/// and takes one second because CPU usage is computed from two `/proc/stat` samples.
///
/// Wrapped in `sh -c '...'` so it behaves the same whatever the account's login shell is.
/// The script itself must not contain a single quote, `$` or a backtick (checked by a test).
pub const COMMAND: &str = concat!(
    "sh -c '",
    "echo \"##CPU1\"; head -n 1 /proc/stat; ",
    "sleep 1; ",
    "echo \"##CPU2\"; head -n 1 /proc/stat; ",
    "echo \"##MEM\"; grep -E \"^(MemTotal|MemFree|MemAvailable|Buffers|Cached|SwapTotal|SwapFree):\" /proc/meminfo; ",
    "echo \"##DISK\"; df -P -k / | tail -n 1; ",
    "echo \"##UPTIME\"; cut -d\" \" -f1 /proc/uptime; ",
    "echo \"##LOAD\"; cut -d\" \" -f1-3 /proc/loadavg; ",
    "echo \"##CORES\"; getconf _NPROCESSORS_ONLN",
    "'"
);

/// Remote output larger than this is rejected.
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    /// 0-100, over a one second window.
    pub cpu_percent: f64,
    pub cores: u32,
    pub load1: f64,
    pub load5: f64,
    pub load15: f64,
    pub mem_total_kib: u64,
    pub mem_available_kib: u64,
    pub mem_used_percent: f64,
    pub swap_total_kib: u64,
    pub swap_used_kib: u64,
    /// The filesystem mounted at `/`.
    pub disk_total_kib: u64,
    pub disk_used_kib: u64,
    pub disk_used_percent: f64,
    pub uptime_secs: u64,
    /// Unix epoch milliseconds, set by VPSPilot (not the remote clock).
    pub sampled_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(what: &str) -> Result<T, ParseError> {
    Err(ParseError(format!(
        "The server returned unexpected output ({what})."
    )))
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[derive(Default)]
struct Sections<'a> {
    cpu1: Vec<&'a str>,
    cpu2: Vec<&'a str>,
    mem: Vec<&'a str>,
    disk: Vec<&'a str>,
    uptime: Vec<&'a str>,
    load: Vec<&'a str>,
    cores: Vec<&'a str>,
}

fn split_sections(output: &str) -> Sections<'_> {
    let mut s = Sections::default();
    let mut current: Option<&str> = None;
    for line in output.lines().map(str::trim_end) {
        if let Some(name) = line.strip_prefix("##") {
            current = Some(name);
            continue;
        }
        let target = match current {
            Some("CPU1") => &mut s.cpu1,
            Some("CPU2") => &mut s.cpu2,
            Some("MEM") => &mut s.mem,
            Some("DISK") => &mut s.disk,
            Some("UPTIME") => &mut s.uptime,
            Some("LOAD") => &mut s.load,
            Some("CORES") => &mut s.cores,
            _ => continue,
        };
        if !line.is_empty() {
            target.push(line);
        }
    }
    s
}

/// (total, idle) jiffies from the aggregate `cpu` line of /proc/stat.
fn cpu_counters(lines: &[&str]) -> Result<(u64, u64), ParseError> {
    let line = lines.first().copied().unwrap_or("");
    let mut parts = line.split_whitespace();
    if parts.next() != Some("cpu") {
        return err("cpu");
    }
    let nums: Vec<u64> = parts.map_while(|p| p.parse().ok()).collect();
    if nums.len() < 4 {
        return err("cpu");
    }
    // user nice system idle iowait irq softirq steal (guest time is already in user)
    let total: u64 = nums.iter().take(8).sum();
    let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
    Ok((total, idle))
}

fn cpu_percent(cpu1: &[&str], cpu2: &[&str]) -> Result<f64, ParseError> {
    let (t1, i1) = cpu_counters(cpu1)?;
    let (t2, i2) = cpu_counters(cpu2)?;
    if t2 < t1 || i2 < i1 {
        return err("cpu counters went backwards");
    }
    let dt = t2 - t1;
    if dt == 0 {
        return Ok(0.0);
    }
    let busy = dt.saturating_sub(i2 - i1);
    Ok(round1((busy as f64 / dt as f64 * 100.0).clamp(0.0, 100.0)))
}

struct Mem {
    total: u64,
    available: u64,
    swap_total: u64,
    swap_used: u64,
}

fn meminfo(lines: &[&str]) -> Result<Mem, ParseError> {
    let get = |key: &str| -> Option<u64> {
        lines.iter().find_map(|l| {
            let rest = l.strip_prefix(key)?.strip_prefix(':')?;
            rest.split_whitespace().next()?.parse().ok()
        })
    };
    let Some(total) = get("MemTotal").filter(|t| *t > 0) else {
        return err("memory");
    };
    // Kernels before 3.14 have no MemAvailable.
    let available = match get("MemAvailable") {
        Some(a) => a,
        None => match get("MemFree") {
            Some(f) => f + get("Buffers").unwrap_or(0) + get("Cached").unwrap_or(0),
            None => return err("memory"),
        },
    };
    let swap_total = get("SwapTotal").unwrap_or(0);
    let swap_free = get("SwapFree").unwrap_or(0);
    Ok(Mem {
        total,
        available: available.min(total),
        swap_total,
        swap_used: swap_total.saturating_sub(swap_free),
    })
}

/// Parses `df -P -k /` data line from the right, since the filesystem name may contain spaces:
/// `<fs> <total> <used> <available> <capacity%> <mount>`.
fn disk(lines: &[&str]) -> Result<(u64, u64, f64), ParseError> {
    let line = lines.last().copied().unwrap_or("");
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() < 6 {
        return err("disk");
    }
    let n = f.len();
    let (Ok(total), Ok(used), Ok(avail)) = (
        f[n - 5].parse::<u64>(),
        f[n - 4].parse::<u64>(),
        f[n - 3].parse::<u64>(),
    ) else {
        return err("disk");
    };
    let denom = used + avail;
    let percent = if denom == 0 {
        0.0
    } else {
        round1(used as f64 / denom as f64 * 100.0)
    };
    Ok((total, used, percent))
}

fn first_f64(lines: &[&str], what: &str) -> Result<f64, ParseError> {
    match lines
        .first()
        .and_then(|l| l.split_whitespace().next()?.parse::<f64>().ok())
    {
        Some(v) if v.is_finite() && v >= 0.0 => Ok(v),
        _ => err(what),
    }
}

fn load(lines: &[&str]) -> Result<(f64, f64, f64), ParseError> {
    let nums: Vec<f64> = lines
        .first()
        .map(|l| {
            l.split_whitespace()
                .map_while(|p| p.parse::<f64>().ok())
                .collect()
        })
        .unwrap_or_default();
    match nums[..] {
        [a, b, c, ..] if [a, b, c].iter().all(|v| v.is_finite() && *v >= 0.0) => Ok((a, b, c)),
        _ => err("load"),
    }
}

/// Parses the full output of [`COMMAND`]. `now_ms` stamps the sample.
pub fn parse_output(output: &str, now_ms: i64) -> Result<Metrics, ParseError> {
    if output.len() > MAX_OUTPUT_BYTES {
        return err("too much output");
    }
    let s = split_sections(output);

    let cpu = cpu_percent(&s.cpu1, &s.cpu2)?;
    let mem = meminfo(&s.mem)?;
    let (disk_total, disk_used, disk_percent) = disk(&s.disk)?;
    let uptime = first_f64(&s.uptime, "uptime")?;
    let (l1, l5, l15) = load(&s.load)?;
    let cores = match s.cores.first().and_then(|l| l.trim().parse::<u32>().ok()) {
        Some(c) if (1..=100_000).contains(&c) => c,
        _ => return err("cores"),
    };

    Ok(Metrics {
        cpu_percent: cpu,
        cores,
        load1: round1(l1 * 100.0) / 100.0,
        load5: round1(l5 * 100.0) / 100.0,
        load15: round1(l15 * 100.0) / 100.0,
        mem_total_kib: mem.total,
        mem_available_kib: mem.available,
        mem_used_percent: round1((mem.total - mem.available) as f64 / mem.total as f64 * 100.0),
        swap_total_kib: mem.swap_total,
        swap_used_kib: mem.swap_used,
        disk_total_kib: disk_total,
        disk_used_kib: disk_used,
        disk_used_percent: disk_percent,
        uptime_secs: uptime as u64,
        sampled_at: now_ms,
    })
}

/// A realistic output of [`COMMAND`], used by tests here and by the fake SSH server.
#[cfg(test)]
pub const SAMPLE_OUTPUT: &str = "##CPU1
cpu  1000 0 500 8000 100 0 0 0 0 0
##CPU2
cpu  1100 0 550 8100 100 0 0 0 0 0
##MEM
MemTotal:        2048000 kB
MemFree:          300000 kB
MemAvailable:    1024000 kB
Buffers:           50000 kB
Cached:           600000 kB
SwapTotal:        512000 kB
SwapFree:         412000 kB
##DISK
/dev/vda1       41151808 12345678  26701898      32% /
##UPTIME
93784.52
##LOAD
0.52 0.40 0.31
##CORES
2
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_is_constant_read_only_and_quote_safe() {
        let script = COMMAND
            .strip_prefix("sh -c '")
            .and_then(|s| s.strip_suffix('\''))
            .expect("wrapped in sh -c");
        for bad in ['\'', '$', '`', '\n', '>', '<', '&', '|'] {
            // A pipe is used for `df | tail`; everything else must be absent.
            if bad == '|' {
                continue;
            }
            assert!(!script.contains(bad), "script must not contain {bad:?}");
        }
        for bad in ["sudo", "rm ", "kill", "chmod", "curl", "wget", "tee", "dd "] {
            assert!(!COMMAND.contains(bad), "{bad}");
        }
    }

    #[test]
    fn parses_a_realistic_sample() {
        let m = parse_output(SAMPLE_OUTPUT, 42).unwrap();
        // delta total = 100+0+50+100+0 = 250 ; delta idle = 100 ; busy = 150 -> 60%
        assert_eq!(m.cpu_percent, 60.0);
        assert_eq!(m.cores, 2);
        assert_eq!((m.load1, m.load5, m.load15), (0.52, 0.4, 0.31));
        assert_eq!(m.mem_total_kib, 2_048_000);
        assert_eq!(m.mem_available_kib, 1_024_000);
        assert_eq!(m.mem_used_percent, 50.0);
        assert_eq!((m.swap_total_kib, m.swap_used_kib), (512_000, 100_000));
        assert_eq!(
            (m.disk_total_kib, m.disk_used_kib),
            (41_151_808, 12_345_678)
        );
        assert_eq!(m.disk_used_percent, 31.6); // used / (used + available), like df
        assert_eq!(m.uptime_secs, 93_784);
        assert_eq!(m.sampled_at, 42);
    }

    #[test]
    fn falls_back_when_memavailable_is_missing() {
        let out = SAMPLE_OUTPUT.replace("MemAvailable:    1024000 kB\n", "");
        let m = parse_output(&out, 0).unwrap();
        assert_eq!(m.mem_available_kib, 300_000 + 50_000 + 600_000);
    }

    #[test]
    fn disk_filesystem_name_may_contain_spaces() {
        let out = SAMPLE_OUTPUT.replace(
            "/dev/vda1       41151808 12345678  26701898      32% /",
            "my weird fs 1000 400 600 40% /",
        );
        let m = parse_output(&out, 0).unwrap();
        assert_eq!(
            (m.disk_total_kib, m.disk_used_kib, m.disk_used_percent),
            (1000, 400, 40.0)
        );
    }

    #[test]
    fn no_swap_is_fine() {
        let out = SAMPLE_OUTPUT
            .replace(
                "SwapTotal:        512000 kB\n",
                "SwapTotal:             0 kB\n",
            )
            .replace(
                "SwapFree:         412000 kB\n",
                "SwapFree:              0 kB\n",
            );
        let m = parse_output(&out, 0).unwrap();
        assert_eq!((m.swap_total_kib, m.swap_used_kib), (0, 0));
    }

    #[test]
    fn idle_machine_and_fully_busy_machine_are_clamped_correctly() {
        let idle = SAMPLE_OUTPUT.replace(
            "cpu  1100 0 550 8100 100 0 0 0 0 0",
            "cpu  1000 0 500 9000 100 0 0 0 0 0",
        );
        assert_eq!(parse_output(&idle, 0).unwrap().cpu_percent, 0.0);
        let busy = SAMPLE_OUTPUT.replace(
            "cpu  1100 0 550 8100 100 0 0 0 0 0",
            "cpu  2000 0 500 8000 100 0 0 0 0 0",
        );
        assert_eq!(parse_output(&busy, 0).unwrap().cpu_percent, 100.0);
    }

    #[test]
    fn zero_elapsed_ticks_is_zero_percent_not_a_panic() {
        let out = SAMPLE_OUTPUT.replace(
            "cpu  1100 0 550 8100 100 0 0 0 0 0",
            "cpu  1000 0 500 8000 100 0 0 0 0 0",
        );
        assert_eq!(parse_output(&out, 0).unwrap().cpu_percent, 0.0);
    }

    #[test]
    fn rejects_counters_going_backwards() {
        let out = SAMPLE_OUTPUT.replace(
            "cpu  1100 0 550 8100 100 0 0 0 0 0",
            "cpu  1 0 1 1 1 0 0 0 0 0",
        );
        assert!(parse_output(&out, 0).is_err());
    }

    #[test]
    fn rejects_missing_or_garbled_sections_with_a_safe_message() {
        for needle in ["##MEM", "##DISK", "##UPTIME", "##LOAD", "##CORES", "##CPU2"] {
            let out = SAMPLE_OUTPUT.replace(needle, "##IGNORED");
            let e = parse_output(&out, 0).unwrap_err();
            assert!(
                e.0.starts_with("The server returned unexpected output"),
                "{needle}: {e}"
            );
        }
        assert!(parse_output("", 0).is_err());
        assert!(parse_output("sh: 1: head: not found\n", 0).is_err());
        assert!(parse_output(&SAMPLE_OUTPUT.replace("2048000", "NaN"), 0).is_err());
        assert!(parse_output(&SAMPLE_OUTPUT.replace("2048000", "0"), 0).is_err());
        assert!(parse_output(&SAMPLE_OUTPUT.replace("\n2\n", "\n0\n"), 0).is_err());
        assert!(parse_output(&SAMPLE_OUTPUT.replace("0.52 0.40 0.31", "a b c"), 0).is_err());
        assert!(parse_output(&SAMPLE_OUTPUT.replace("93784.52", "-5"), 0).is_err());
    }

    #[test]
    fn error_messages_never_echo_remote_output() {
        let e = parse_output("SECRET-REMOTE-TEXT", 0).unwrap_err();
        assert!(!e.0.contains("SECRET"));
    }

    #[test]
    fn ignores_unknown_sections_and_extra_lines() {
        let out = format!("##EVIL\nrm -rf /\n{SAMPLE_OUTPUT}##MORE\nwhatever\n");
        assert!(parse_output(&out, 0).is_ok());
    }

    #[test]
    fn rejects_oversized_output() {
        let big = format!("{SAMPLE_OUTPUT}{}", "x".repeat(MAX_OUTPUT_BYTES));
        assert!(parse_output(&big, 0).is_err());
    }

    #[test]
    fn mem_available_larger_than_total_is_capped() {
        let out =
            SAMPLE_OUTPUT.replace("MemAvailable:    1024000 kB", "MemAvailable:    9999999 kB");
        let m = parse_output(&out, 0).unwrap();
        assert_eq!(m.mem_used_percent, 0.0);
    }
}
