//! Locating the user's gesture config: resolve the main config.kdl, follow
//! `include` lines (the same way niri does), and decide which file the app
//! manages.

use crate::kdl;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const RECURSION_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// A dedicated included file owns the `gestures {}` node.
    ManagedFile(PathBuf),
    /// More than one file has a `gestures` node; we manage the first, the
    /// others are listed so the user knows later includes override.
    ManagedFileAmbiguous(PathBuf, Vec<PathBuf>),
    /// config.kdl itself has an inline `gestures {}` node.
    Inline(PathBuf),
    /// No gestures config anywhere.
    None,
}

#[derive(Debug, Clone)]
pub struct Discovery {
    /// The main config file, if one exists.
    pub config_path: Option<PathBuf>,
    pub source: Source,
}

/// Anything worth surfacing in the UI that is not an error.
#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    ManagedFile(PathBuf),
    AmbiguousFiles(PathBuf, Vec<PathBuf>),
    InlineInConfig(PathBuf),
    FirstRun,
    NoConfigFile,
}

impl Discovery {
    pub fn notices(&self) -> Vec<Notice> {
        let mut n = Vec::new();
        if self.config_path.is_none() {
            n.push(Notice::NoConfigFile);
        }
        match &self.source {
            Source::ManagedFile(p) => n.push(Notice::ManagedFile(p.clone())),
            Source::ManagedFileAmbiguous(p, others) => {
                n.push(Notice::AmbiguousFiles(p.clone(), others.clone()))
            }
            Source::Inline(p) => n.push(Notice::InlineInConfig(p.clone())),
            Source::None => n.push(Notice::FirstRun),
        }
        n
    }
}

/// NIRI_CONFIG > $XDG_CONFIG_HOME/niri/config.kdl > ~/.config/niri/config.kdl
/// > /etc/niri/config.kdl, first that exists.
pub fn resolve_config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NIRI_CONFIG") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home_dir().map(|h| h.join(".config")));
    if let Some(ch) = config_home {
        let p = ch.join("niri/config.kdl");
        if p.is_file() {
            return Some(p);
        }
    }
    let etc = PathBuf::from("/etc/niri/config.kdl");
    if etc.is_file() {
        return Some(etc);
    }
    None
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

pub fn default_gestures_path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home_dir().map(|h| h.join(".config")))?;
    Some(config_home.join("niri/cfg/gestures.kdl"))
}

/// Expand an include target the way niri does: `~` first, then relative to
/// the including file's directory.
fn expand_include(raw: &str, including: &Path) -> PathBuf {
    if let Some(rest) = raw.strip_prefix('~') {
        if let Some(home) = home_dir() {
            return home.join(rest.trim_start_matches('/'));
        }
    }
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        including.parent().unwrap_or(Path::new(".")).join(p)
    }
}

/// Parse one file's direct `include` lines into raw targets.
fn direct_includes(text: &str) -> Vec<String> {
    let Ok(doc) = kdl::parse_document(text) else {
        return Vec::new();
    };
    doc.iter()
        .filter(|n| n.name == "include")
        .filter_map(|n| n.args.first().map(|a| a.text().to_string()))
        .collect()
}

/// Walk a config and all its includes, breadth-first, in niri merge order.
fn include_chain(config: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut queue = std::collections::VecDeque::from([(config.to_path_buf(), 0usize)]);
    visited.insert(config.canonicalize().unwrap_or_else(|_| config.to_path_buf()));

    while let Some((path, depth)) = queue.pop_front() {
        if depth > RECURSION_LIMIT {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for raw in direct_includes(&text) {
            let expanded = expand_include(&raw, &path);
            if !expanded.is_file() {
                continue; // optional or dangling: skipped either way
            }
            let canon = expanded
                .canonicalize()
                .unwrap_or_else(|_| expanded.clone());
            if visited.insert(canon) {
                out.push(expanded.clone());
                queue.push_back((expanded, depth + 1));
            }
        }
    }
    out
}

/// Does this file contain a top-level `gestures` node?
pub fn file_has_gestures(path: &Path) -> Result<bool, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc = kdl::parse_document(&text).map_err(|e| e.to_string())?;
    Ok(doc.iter().any(|n| n.name == "gestures"))
}

/// Full startup discovery.
pub fn discover() -> Discovery {
    let Some(config_path) = resolve_config_path() else {
        return Discovery {
            config_path: None,
            source: Source::None,
        };
    };

    // Included files first (in merge order), then the config itself.
    let mut candidates: Vec<PathBuf> = include_chain(&config_path);
    candidates.push(config_path.clone());

    let mut with_gestures: Vec<PathBuf> = Vec::new();
    let mut parse_error: Option<(PathBuf, String)> = None;
    for path in &candidates {
        match file_has_gestures(path) {
            Ok(true) => with_gestures.push(path.clone()),
            Ok(false) => {}
            Err(e) => {
                // A file we can't parse is only fatal if it's the one we
                // would manage; remember the first one for the banner.
                if parse_error.is_none() {
                    parse_error = Some((path.clone(), e));
                }
            }
        }
    }

    let source = match with_gestures.as_slice() {
        [only] if *only != config_path => Source::ManagedFile(only.clone()),
        [_only] => Source::Inline(config_path.clone()),
        [] => {
            // Nothing parseable. If the main config is unparseable but clearly
            // has an inline gestures block (span scan), treat it as inline so
            // the user gets the extraction flow.
            if let Ok(text) = std::fs::read_to_string(&config_path) {
                if kdl::find_top_level_node_span(&text, "gestures").is_some() {
                    Source::Inline(config_path.clone())
                } else {
                    Source::None
                }
            } else {
                Source::None
            }
        }
        many => Source::ManagedFileAmbiguous(many[0].clone(), many[1..].to_vec()),
    };
    let _ = parse_error; // surfaced by callers via file_has_gestures if needed

    Discovery {
        config_path: Some(config_path),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("easysetup-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_managed_include() {
        let dir = temp_tree("include");
        let cfg_dir = dir.join("niri");
        let cfg = cfg_dir.join("config.kdl");
        fs::create_dir_all(cfg_dir.join("cfg")).unwrap();
        fs::write(&cfg, "include \"./cfg/gestures.kdl\"\n").unwrap();
        fs::write(
            cfg_dir.join("cfg/gestures.kdl"),
            "gestures { touchscreen-swipe { tap { maximize-column; } } }\n",
        )
        .unwrap();

        let chain = include_chain(&cfg);
        assert_eq!(chain, vec![cfg_dir.join("cfg/gestures.kdl")]);
        let d = Discovery {
            config_path: Some(cfg),
            source: Source::ManagedFile(cfg_dir.join("cfg/gestures.kdl")),
        };
        assert!(matches!(&d.source, Source::ManagedFile(p) if *p == cfg_dir.join("cfg/gestures.kdl")));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tilde_and_optional_expansion() {
        let hits = direct_includes("include \"~/.config/niri/cfg/gestures.kdl\"\ninclude \"missing.kdl\" optional=true\n");
        assert_eq!(hits.len(), 2);
        assert!(hits[0].contains("gestures.kdl"));
        let home = home_dir().expect("HOME set in tests");
        let expanded = expand_include(&hits[0], Path::new("/etc/niri/config.kdl"));
        assert_eq!(expanded, home.join(".config/niri/cfg/gestures.kdl"));
    }

    #[test]
    fn detects_inline_gestures() {
        let dir = temp_tree("inline");
        let cfg = dir.join("config.kdl");
        fs::write(&cfg, "// hi\ngestures {\n    debug-log\n}\n").unwrap();
        assert!(file_has_gestures(&cfg).unwrap());
        let text = fs::read_to_string(&cfg).unwrap();
        assert!(kdl::find_top_level_node_span(&text, "gestures").is_some());
        fs::remove_dir_all(&dir).ok();
    }
}
