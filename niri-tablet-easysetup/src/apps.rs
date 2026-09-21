//! Enumerating installed applications from .desktop files, the way app
//! launchers do, so gestures can spawn real apps by name.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct AppEntry {
    /// Desktop file basename, e.g. "org.gnome.Settings.desktop".
    pub id: String,
    pub name: String,
    /// argv for `spawn` (field codes like %f stripped).
    pub argv: Vec<String>,
    pub icon: Option<String>,
}

#[derive(Default)]
struct RawEntry {
    name: Option<String>,
    exec: Option<String>,
    icon: Option<String>,
    no_display: bool,
    hidden: bool,
    terminal: bool,
    only_show_in: Option<Vec<String>>,
    not_show_in: Option<Vec<String>>,
    try_exec: Option<String>,
}

/// XDG data dirs' applications/ folders, most-specific last so user entries
/// win on dedup.
fn app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share"))
        });
    let data_dirs: Vec<PathBuf> = std::env::var_os("XDG_DATA_DIRS")
        .map(|d| std::env::split_paths(&d).collect())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });
    // System dirs first, then the user dir (later wins in our dedup map).
    for d in data_dirs {
        dirs.push(d.join("applications"));
    }
    if let Some(dh) = data_home {
        dirs.push(dh.join("applications"));
    }
    dirs
}

fn parse_desktop(text: &str) -> RawEntry {
    let mut e = RawEntry::default();
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        // Skip localized keys (Name[pt]=...) and friends.
        if k.contains('[') {
            continue;
        }
        match k {
            "Name" => e.name = Some(v.to_string()),
            "Exec" => e.exec = Some(v.to_string()),
            "Icon" => e.icon = Some(v.to_string()),
            "NoDisplay" => e.no_display = v.eq_ignore_ascii_case("true"),
            "Hidden" => e.hidden = v.eq_ignore_ascii_case("true"),
            "Terminal" => e.terminal = v.eq_ignore_ascii_case("true"),
            "OnlyShowIn" => e.only_show_in = Some(split_list(v)),
            "NotShowIn" => e.not_show_in = Some(split_list(v)),
            "TryExec" => e.try_exec = Some(v.to_string()),
            _ => {}
        }
    }
    e
}

fn split_list(v: &str) -> Vec<String> {
    v.split(';').filter_map(|s| {
        let s = s.trim();
        if s.is_empty() { None } else { Some(s.to_string()) }
    })
    .collect()
}

/// Exec value -> argv: shell-ish split, then drop desktop field codes
/// (standalone tokens starting with `%`).
fn exec_to_argv(exec: &str) -> Option<Vec<String>> {
    let words = shlex::split(exec)?;
    let argv: Vec<String> = words
        .into_iter()
        .filter(|w| !w.starts_with('%'))
        .collect();
    if argv.is_empty() { None } else { Some(argv) }
}

fn binary_on_path(bin: &str) -> bool {
    if bin.contains('/') {
        return std::path::Path::new(bin).is_file();
    }
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|d| d.join(bin).is_file())
        })
        .unwrap_or(false)
}

fn current_desktops() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| split_list(&v.replace(':', ";")))
        .unwrap_or_default()
}

pub fn scan() -> Vec<AppEntry> {
    let mut by_id: BTreeMap<String, AppEntry> = BTreeMap::new();
    let desktops = current_desktops();

    for dir in app_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(id) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let raw = parse_desktop(&text);
            if raw.no_display || raw.hidden || raw.terminal {
                continue;
            }
            if let Some(only) = &raw.only_show_in {
                if !only.iter().any(|d| desktops.iter().any(|c| c == d)) {
                    continue;
                }
            }
            if let Some(not) = &raw.not_show_in {
                if not.iter().any(|d| desktops.iter().any(|c| c == d)) {
                    continue;
                }
            }
            if let Some(try_exec) = &raw.try_exec {
                if !binary_on_path(try_exec) {
                    continue;
                }
            }
            let (Some(name), Some(exec)) = (&raw.name, &raw.exec) else {
                continue;
            };
            let Some(argv) = exec_to_argv(exec) else {
                continue;
            };
            if !binary_on_path(&argv[0]) {
                // Spawning would just fail; hide it. (Scripts with shebangs
                // pass is_file() above only if absolute; PATH check covers the rest.)
                continue;
            }
            by_id.insert(
                id.clone(),
                AppEntry {
                    id,
                    name: name.clone(),
                    argv,
                    icon: raw.icon.clone(),
                },
            );
        }
    }

    let mut apps: Vec<AppEntry> = by_id.into_values().collect();
    // Same app installed twice (system + user): dedup by name+argv, keep user.
    let mut seen = std::collections::HashSet::new();
    apps.retain(|a| seen.insert((a.name.clone(), a.argv.clone())));
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_field_codes_stripped() {
        let argv = exec_to_argv("fuzzel --log /tmp/f %f").unwrap();
        assert_eq!(argv, vec!["fuzzel", "--log", "/tmp/f"]);
        assert!(exec_to_argv("%f").is_none());
        // Quoted arg with spaces.
        let argv = exec_to_argv("env FOO=\"a b\" foot").unwrap();
        assert_eq!(argv, vec!["env", "FOO=a b", "foot"]);
    }

    #[test]
    fn parses_a_real_shaped_entry() {
        let text = "[Desktop Entry]\nType=Application\nName=Foot Terminal\nName[pt]=Terminal\nExec=foot -e %u\nIcon=foot\nTerminal=false\n\n[Other Group]\nName=Ignored\n";
        let e = parse_desktop(text);
        assert_eq!(e.name.as_deref(), Some("Foot Terminal"));
        assert_eq!(e.icon.as_deref(), Some("foot"));
        assert!(!e.terminal);
    }

    #[test]
    fn nodelay_hidden_and_onlyshowin_filter() {
        let mut e = parse_desktop("[Desktop Entry]\nName=X\nExec=x\nNoDisplay=true\n");
        assert!(e.no_display);
        e = parse_desktop("[Desktop Entry]\nName=X\nExec=x\nOnlyShowIn=GNOME;\n");
        assert_eq!(e.only_show_in.as_deref(), Some(&["GNOME".to_string()][..]));
    }
}
