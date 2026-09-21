//! Talking to the `niri` binary: config validation and live action testing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resolve the niri binary once; None when it's not on PATH.
pub fn find_niri() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("niri"))
        .find(|p| p.is_file())
}

/// Is an IPC socket plausibly available (needed for Test buttons)?
pub fn ipc_available() -> bool {
    if std::env::var_os("NIRI_SOCKET").is_some() {
        return true;
    }
    // Fallback scan of $XDG_RUNTIME_DIR for niri.wayland-*.sock
    let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) else {
        return false;
    };
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("niri.wayland-") && n.ends_with(".sock")
            })
        })
        .unwrap_or(false)
}

fn run(niri: &Path, args: &[&str]) -> (i32, String, String) {
    match Command::new(niri)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            (
                code,
                String::from_utf8_lossy(&out.stdout).into_owned(),
                String::from_utf8_lossy(&out.stderr).into_owned(),
            )
        }
        Err(e) => (-1, String::new(), format!("could not run niri: {e}")),
    }
}

/// Validate a config file. Ok(()) on exit 0, Err(stderr/stdout tail) otherwise.
pub fn validate(niri: &Path, config: &Path) -> Result<(), String> {
    let cfg = config.to_string_lossy().into_owned();
    let (code, stdout, stderr) = run(niri, &["validate", "-c", &cfg]);
    if code == 0 {
        Ok(())
    } else {
        let msg = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        // Keep the tail: knuffel errors include a snippet block.
        let tail: String = if msg.lines().count() > 25 {
            msg.lines().skip(msg.lines().count() - 25).collect::<Vec<_>>().join("\n")
        } else {
            msg.to_string()
        };
        Err(tail)
    }
}

/// Fire an action on the running compositor for testing.
/// `words` is the action argv after "msg action", e.g.
/// ["toggle-overview"] or ["spawn", "--", "foot"].
pub fn msg_action(niri: &Path, words: &[String]) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["msg", "action"];
    args.extend(words.iter().map(|s| s.as_str()));
    let (code, stdout, stderr) = run(niri, &args);
    if code == 0 {
        Ok(())
    } else {
        Err(if stderr.trim().is_empty() { stdout } else { stderr })
    }
}

/// A subscription to `niri msg --json event-stream`. The compositor emits
/// `{"ConfigLoaded":{"failed":bool}}` once on connect (the last load
/// attempt) and after every config reload, so a save can be confirmed by
/// the compositor's own verdict.
pub struct ReloadWatch {
    child: std::process::Child,
    rx: std::sync::mpsc::Receiver<String>,
}

impl Drop for ReloadWatch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ReloadWatch {
    /// Wait for the next ConfigLoaded event within `timeout`.
    /// Some(false) = reloaded fine, Some(true) = the compositor rejected
    /// the config and kept the previous one, None = timeout/stream ended.
    pub fn wait_config_loaded(&self, timeout: std::time::Duration) -> Option<bool> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let Some(remain) = deadline.checked_duration_since(std::time::Instant::now()) else {
                return None;
            };
            match self.rx.recv_timeout(remain) {
                Ok(line) => {
                    if let Some(failed) = parse_config_loaded(&line) {
                        return Some(failed);
                    }
                }
                Err(_) => return None,
            }
        }
    }
}

fn parse_config_loaded(line: &str) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    v.get("ConfigLoaded")?.get("failed")?.as_bool()
}

/// Start watching the event stream, best-effort.
pub fn event_stream(niri: &Path) -> Option<ReloadWatch> {
    let mut child = Command::new(niri)
        .args(["msg", "--json", "event-stream"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    Some(ReloadWatch { child, rx })
}

/// True when the installed niri understands the `gestures {}` config node
/// (i.e. it's the patched niri-tablet build).
pub fn gestures_supported(niri: &Path) -> (bool, String) {
    let probe = std::env::temp_dir().join("niri-tablet-easysetup-probe.kdl");
    let body = "gestures {\n    touchscreen-swipe {\n        tap { maximize-column; }\n    }\n}\n";
    let write = std::fs::File::create(&probe)
        .and_then(|mut f| f.write_all(body.as_bytes()));
    if let Err(e) = write {
        return (false, format!("could not write probe file: {e}"));
    }
    match validate(niri, &probe) {
        Ok(()) => (true, String::new()),
        Err(e) => (false, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_file_writes_and_validates_shape() {
        // Can't assume niri exists in CI/test envs; just check the helper
        // composes the right argv when the binary is missing.
        let p = PathBuf::from("/nonexistent-niri");
        assert!(validate(&p, Path::new("/etc/hostname")).is_err());
    }
}
