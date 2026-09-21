//! Loading and saving the managed gestures file.
//!
//! Safety invariant: the rendered config is validated against a scratch file
//! in the system temp dir BEFORE any real file is touched. Only validated
//! content ever reaches the user's config directory; every write into it is
//! a same-filesystem temp-then-rename so niri's 500 ms watcher never sees a
//! partial file.

use crate::discovery::{self, Discovery, Source};
use crate::kdl;
use crate::model::GestureModel;
use crate::niri;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BACKUP_SUFFIX: &str = ".easysetup.bak";

pub struct Loaded {
    pub discovery: Discovery,
    pub model: GestureModel,
    pub warnings: Vec<String>,
}

/// Startup load: discovery + parse whatever we found.
pub fn load() -> Loaded {
    let d = discovery::discover();
    let mut warnings = Vec::new();

    let model = match &d.source {
        Source::ManagedFile(p) | Source::ManagedFileAmbiguous(p, _) => {
            match std::fs::read_to_string(p) {
                Ok(text) => match GestureModel::parse_file_text(&text) {
                    Ok(Some(m)) => m,
                    Ok(None) => {
                        warnings.push(format!(
                            "{} has no gestures node; starting from an empty scheme",
                            p.display()
                        ));
                        GestureModel::default()
                    }
                    Err(e) => {
                        warnings.push(format!(
                            "could not parse {} ({e}); showing an empty scheme until it's fixed",
                            p.display()
                        ));
                        GestureModel::default()
                    }
                },
                Err(e) => {
                    warnings.push(format!("could not read {}: {e}", p.display()));
                    GestureModel::default()
                }
            }
        }
        Source::Inline(p) => match std::fs::read_to_string(p) {
            Ok(text) => match GestureModel::parse_file_text(&text) {
                Ok(Some(m)) => m,
                _ => {
                    warnings.push(format!(
                        "could not parse the gestures block in {}; showing an empty scheme",
                        p.display()
                    ));
                    GestureModel::default()
                }
            },
            Err(e) => {
                warnings.push(format!("could not read {}: {e}", p.display()));
                GestureModel::default()
            }
        },
        Source::None => GestureModel::default(),
    };

    Loaded {
        discovery: d,
        model,
        warnings,
    }
}

#[derive(Debug)]
pub enum SaveError {
    /// There is no config.kdl and no place to put the gestures file.
    NoConfig,
    Io(String),
    /// niri validate rejected the result; nothing was written.
    ValidateFailed(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NoConfig => write!(f, "no niri config found to add an include to"),
            SaveError::Io(e) => write!(f, "{e}"),
            SaveError::ValidateFailed(e) => write!(f, "niri rejected the new config:\n{e}"),
        }
    }
}

/// Save outcome for the UI.
pub enum Saved {
    Written { path: PathBuf },
}

/// Apply the model to disk.
///
/// - ManagedFile: backup once, then atomically replace the managed file.
/// - Inline: extract first (config.kdl gets a one-time backup, the gestures
///   node is replaced by an include line), then the managed file is written.
/// - None: first run: create the managed file and append the include line to
///   config.kdl (backed up first).
pub fn save(d: &Discovery, model: &GestureModel) -> Result<Saved, SaveError> {
    // 1. Render and validate against a scratch file. Nothing below runs
    //    unless this passes (or niri is absent, in which case the caller
    //    reports the save as unvalidated).
    let text = model.to_file_text();
    let niri_bin = niri::find_niri();
    validate_rendered(&text, niri_bin.as_deref())?;

    let target = match &d.source {
        Source::ManagedFile(p) | Source::ManagedFileAmbiguous(p, _) => {
            write_atomic(p, &text)?;
            p.clone()
        }
        Source::Inline(config) => {
            let target = default_target().ok_or(SaveError::NoConfig)?;
            extract_inline_at(config, &target, &text)?;
            target
        }
        Source::None => {
            let config = d.config_path.clone().ok_or(SaveError::NoConfig)?;
            let target = default_target()
                .ok_or_else(|| SaveError::Io("could not determine a config directory".into()))?;
            first_run_at(&config, &target, &text)?;
            target
        }
    };

    Ok(Saved::Written { path: target })
}

fn default_target() -> Option<PathBuf> {
    discovery::default_gestures_path()
}

/// Validate rendered config text via a scratch file in the system temp dir.
/// Err = rejected, nothing was touched; Ok(()) = validated (or niri absent,
/// in which case the caller reports the save as unvalidated).
fn validate_rendered(text: &str, niri: Option<&Path>) -> Result<(), SaveError> {
    let Some(niri) = niri else {
        return Ok(());
    };
    let probe = std::env::temp_dir().join(format!(
        "niri-tablet-easysetup-save-{}.kdl",
        std::process::id()
    ));
    let write = std::fs::File::create(&probe).and_then(|mut f| f.write_all(text.as_bytes()));
    if let Err(e) = write {
        return Err(SaveError::Io(format!("writing {}: {e}", probe.display())));
    }
    let result = niri::validate(niri, &probe);
    let _ = std::fs::remove_file(&probe);
    result.map_err(SaveError::ValidateFailed)
}

/// Create the managed gestures file and add the include line to config.kdl.
fn first_run_at(config: &Path, target: &Path, text: &str) -> Result<(), SaveError> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| SaveError::Io(format!("creating {}: {e}", parent.display())))?;
    }
    // The include goes in first so niri never polls a dangling include.
    append_include(config, "./cfg/gestures.kdl")?;
    write_atomic(target, text)
}

/// Move an inline `gestures {}` node out of config.kdl into the managed
/// file, replacing the node with an include line.
fn extract_inline_at(config: &Path, target: &Path, text: &str) -> Result<(), SaveError> {
    let orig = std::fs::read_to_string(config)
        .map_err(|e| SaveError::Io(format!("reading {}: {e}", config.display())))?;
    let Some((start, end)) = kdl::find_top_level_node_span(&orig, "gestures") else {
        return Err(SaveError::Io(
            "the gestures block disappeared from config.kdl; nothing to extract".into(),
        ));
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| SaveError::Io(format!("creating {}: {e}", parent.display())))?;
    }
    write_atomic(target, text)?;

    backup_once(config)?;
    let include_line = "include \"~/.config/niri/cfg/gestures.kdl\"";
    let mut new_text = String::with_capacity(orig.len());
    new_text.push_str(&orig[..start]);
    new_text.push_str(include_line);
    new_text.push_str(&orig[end..]);
    std::fs::write(config, new_text)
        .map_err(|e| SaveError::Io(format!("rewriting {}: {e}", config.display())))
}

/// Append an include line to config.kdl (with a one-time backup).
fn append_include(config: &Path, rel: &str) -> Result<(), SaveError> {
    backup_once(config)?;
    let mut text = std::fs::read_to_string(config)
        .map_err(|e| SaveError::Io(format!("reading {}: {e}", config.display())))?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text.push_str(&format!("include \"{rel}\"\n"));
    std::fs::write(config, text).map_err(|e| SaveError::Io(format!("rewriting {}: {e}", config.display())))
}

/// Copy file -> file.easysetup.bak, but only if no backup exists yet (keep
/// the true pre-app state).
fn backup_once(path: &Path) -> Result<(), SaveError> {
    let bak = backup_path(path);
    if bak.exists() || !path.exists() {
        return Ok(());
    }
    std::fs::copy(path, &bak)
        .map(|_| ())
        .map_err(|e| SaveError::Io(format!("backing up {}: {e}", path.display())))
}

pub fn backup_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(BACKUP_SUFFIX);
    PathBuf::from(s)
}

/// Validated-text atomic replace: write a sibling tmp, rename over.
fn write_atomic(path: &Path, text: &str) -> Result<(), SaveError> {
    let tmp = path.with_extension("kdl.easysetup-tmp");
    std::fs::write(&tmp, text)
        .map_err(|e| SaveError::Io(format!("writing {}: {e}", tmp.display())))?;
    backup_once(path)?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        SaveError::Io(format!("replacing {}: {e}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::shipped_default;
    use std::fs;

    fn temp_tree(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "easysetup-store-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Tests here exercise the *_at helpers with explicit paths only; they
    // never touch the real config directory and never mutate env vars.

    #[test]
    fn first_run_creates_file_and_include() {
        let dir = temp_tree("first-run");
        let config = dir.join("config.kdl");
        fs::write(&config, "// my config\nspawn-at-startup \"waybar\"\n").unwrap();
        let target = dir.join("cfg/gestures.kdl");
        let text = shipped_default().to_file_text();

        first_run_at(&config, &target, &text).unwrap();

        let gestures = fs::read_to_string(&target).unwrap();
        assert!(gestures.contains("maximize-column"));
        assert!(
            !gestures.contains("fingers \"3\""),
            "fingers must be a bare number"
        );
        let cfg_text = fs::read_to_string(&config).unwrap();
        assert!(
            cfg_text.contains("include \"./cfg/gestures.kdl\""),
            "{cfg_text}"
        );
        assert!(cfg_text.contains("waybar"), "original content lost");
        assert!(backup_path(&config).exists(), "config.kdl backup missing");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extraction_moves_inline_node() {
        let dir = temp_tree("extract");
        let config = dir.join("config.kdl");
        fs::write(
            &config,
            "// my config\ninput {\n    keyboard {\n        xkb {\n            layout \"pt,us\"\n        }\n    }\n}\n\ngestures {\n    debug-log\n}\n",
        )
        .unwrap();
        let model = GestureModel::parse_file_text(&fs::read_to_string(&config).unwrap())
            .unwrap()
            .unwrap();
        let target = dir.join("cfg/gestures.kdl");

        extract_inline_at(&config, &target, &model.to_file_text()).unwrap();

        let new_cfg = fs::read_to_string(&config).unwrap();
        assert!(
            !new_cfg.contains("debug-log"),
            "gestures node not removed:\n{new_cfg}"
        );
        assert!(
            new_cfg.contains("layout \"pt,us\""),
            "input block damaged:\n{new_cfg}"
        );
        assert!(new_cfg.contains("include \"~/.config/niri/cfg/gestures.kdl\""));
        let moved = fs::read_to_string(&target).unwrap();
        assert!(moved.contains("debug-log"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validate_rejected_content_touches_nothing() {
        if crate::niri::find_niri().is_none() {
            return; // needs a niri binary to validate with
        }
        let dir = temp_tree("reject");
        let target = dir.join("gestures.kdl");
        fs::write(&target, "gestures { }\n").unwrap();
        // A model whose render fails validation: raw garbage in a slot.
        let mut m = GestureModel::default();
        m.set(
            crate::model::Slot::Tap,
            crate::model::ActionValue::Raw {
                kdl: "tap { \"unterminated }".into(),
            },
        );
        let err = match save(
            &Discovery {
                config_path: Some(dir.join("config.kdl")),
                source: Source::ManagedFile(target.clone()),
            },
            &m,
        ) {
            Err(e) => e,
            Ok(_) => panic!("invalid render must not save"),
        };
        assert!(matches!(err, SaveError::ValidateFailed(_)), "{err}");
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "gestures { }\n",
            "original file must be untouched"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
