//! Export/import of gesture schemes: a versioned JSON snapshot (slots +
//! options; machine-local passthrough nodes are deliberately not exported),
//! plus importing raw .kdl gesture fragments.

use crate::model::{ActionValue, GestureModel, Options, Slot};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: u32 = 2;
pub const APP: &str = "niri-tablet-easysetup";

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Scheme {
    pub schema: u32,
    pub app: String,
    #[serde(default)]
    pub app_version: String,
    pub slots: BTreeMap<String, ActionValue>,
    pub options: Options,
}

impl Slot {
    /// Unique, human-stable key ("tap", "hold-left", "edge-bottom",
    /// "corner-top-left"). The enum's plain KDL ids repeat between sections
    /// (hold left / edge left are both "left"), so exports use these.
    pub fn export_key(&self) -> &'static str {
        match self {
            Slot::Tap => "tap",
            Slot::TapMore => "tap-more",
            Slot::SwipeMoreUp => "swipe-more-up",
            Slot::SwipeMoreDown => "swipe-more-down",
            Slot::HoldLeft => "hold-left",
            Slot::HoldRight => "hold-right",
            Slot::HoldUp => "hold-up",
            Slot::HoldDown => "hold-down",
            Slot::EdgeBottom => "edge-bottom",
            Slot::EdgeTop => "edge-top",
            Slot::EdgeLeft => "edge-left",
            Slot::EdgeRight => "edge-right",
            Slot::CornerTopLeft => "corner-top-left",
            Slot::CornerTopRight => "corner-top-right",
            Slot::CornerBottomLeft => "corner-bottom-left",
            Slot::CornerBottomRight => "corner-bottom-right",
        }
    }

    pub fn from_export_key(k: &str) -> Option<Slot> {
        // Legacy keys from schema 1 (release v26.04.18 and earlier) map to
        // their renamed slots so old scheme files keep importing.
        let legacy = match k {
            "tap-4" => Some(Slot::TapMore),
            "swipe-4-up" => Some(Slot::SwipeMoreUp),
            "swipe-4-down" => Some(Slot::SwipeMoreDown),
            _ => None,
        };
        legacy.or_else(|| Slot::ALL.iter().copied().find(|s| s.export_key() == k))
    }
}

pub fn to_scheme(model: &GestureModel) -> Scheme {
    Scheme {
        schema: SCHEMA,
        app: APP.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        slots: Slot::ALL
            .iter()
            .map(|s| (s.export_key().to_string(), model.get(*s).clone()))
            .collect(),
        options: model.options.clone(),
    }
}

pub struct Imported {
    pub model: GestureModel,
    /// Slot keys present in the file but unknown to this version.
    pub ignored_keys: Vec<String>,
}

/// Import our JSON scheme. Unknown slot keys are ignored (and reported);
/// unknown future schema versions are rejected.
pub fn from_scheme_json(text: &str) -> Result<Imported, String> {
    let scheme: Scheme = serde_json::from_str(text).map_err(|e| format!("not a gesture scheme file: {e}"))?;
    if scheme.schema > SCHEMA {
        return Err(format!(
            "scheme schema {} is newer than this app understands ({SCHEMA}); update niri-tablet-easysetup",
            scheme.schema
        ));
    }
    let mut model = GestureModel::default();
    let mut ignored = Vec::new();
    for (k, v) in scheme.slots {
        match Slot::from_export_key(&k) {
            Some(slot) => model.set(slot, v),
            None => ignored.push(k),
        }
    }
    model.options = scheme.options;
    if let Some(n) = model.options.fingers {
        if !matches!(n, 3 | 4) {
            model.options.fingers = None;
            model
                .warnings
                .push(format!("fingers {n} is not valid (3 or 4); using the default 3"));
        }
    }
    Ok(Imported {
        model,
        ignored_keys: ignored,
    })
}

/// Import a raw KDL file: either a full gestures file (with the `gestures`
/// node) or the same content niri-tablet ships.
pub fn from_kdl_text(text: &str) -> Result<GestureModel, String> {
    match GestureModel::parse_file_text(text)? {
        Some(m) => Ok(m),
        None => Err("no gestures block found in that file".to_string()),
    }
}

/// Serialize to pretty JSON for saving.
pub fn scheme_to_json(model: &GestureModel) -> String {
    serde_json::to_string_pretty(&to_scheme(model)).expect("scheme serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::shipped_default;

    #[test]
    fn json_round_trip() {
        let m = shipped_default();
        let json = scheme_to_json(&m);
        let back = from_scheme_json(&json).unwrap();
        assert_eq!(back.ignored_keys, Vec::<String>::new());
        assert_eq!(back.model.slots, m.slots);
        assert_eq!(back.model.options, m.options);
    }

    #[test]
    fn rejects_future_schema() {
        let json = r#"{"schema": 99, "app": "x", "slots": {}, "options": {}}"#;
        assert!(from_scheme_json(json).is_err());
    }

    #[test]
    fn ignores_unknown_slot_keys() {
        let json = r#"{"schema": 1, "app": "x", "slots": {"telepathy-mode": {"kind": "none"}, "tap": {"kind": "node", "name": "close-window", "args": [], "props": []}}, "options": {}}"#;
        let imp = from_scheme_json(json).unwrap();
        assert_eq!(imp.ignored_keys, vec!["telepathy-mode".to_string()]);
        assert_eq!(imp.model.get(Slot::Tap), &ActionValue::simple("close-window"));
    }

    #[test]
    fn legacy_export_keys_import() {
        // Schema 1 files (release v26.04.18 and earlier) keep importing:
        // old slot keys map to the renamed slots, invalid fingers resets.
        let json = r#"{"schema": 1, "app": "x", "slots": {"tap-4": {"kind": "node", "name": "toggle-overview", "args": [], "props": []}, "swipe-4-down": {"kind": "node", "name": "close-window", "args": [], "props": []}}, "options": {"fingers": 5}}"#;
        let imp = from_scheme_json(json).unwrap();
        assert_eq!(imp.ignored_keys, Vec::<String>::new());
        assert_eq!(imp.model.get(Slot::TapMore), &ActionValue::simple("toggle-overview"));
        assert_eq!(imp.model.get(Slot::SwipeMoreDown), &ActionValue::simple("close-window"));
        assert_eq!(imp.model.options.fingers, None);
        assert_eq!(imp.model.warnings.len(), 1);
    }

    #[test]
    fn kdl_import_takes_gestures_fragment() {
        let m = from_kdl_text(crate::model::SHIPPED_DEFAULT).unwrap();
        assert_eq!(m.get(Slot::Tap), &ActionValue::simple("maximize-column"));
        assert!(from_kdl_text("input {\n}\n").is_err());
    }
}
