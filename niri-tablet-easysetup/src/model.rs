//! The gesture configuration model: the 16 configurable action slots, the
//! mode options, and the mapping to/from the `gestures {}` KDL node.

use crate::kdl::{self, Arg, Node};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The KDL argument literals are tagged in JSON so that bare (`3`) vs quoted
/// (`"3"`) survives an export/import round trip.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum JsonArg {
    #[serde(rename = "s")]
    Str(String),
    #[serde(rename = "w")]
    Word(String),
}

impl From<&Arg> for JsonArg {
    fn from(a: &Arg) -> JsonArg {
        match a {
            Arg::Str(s) => JsonArg::Str(s.clone()),
            Arg::Word(w) => JsonArg::Word(w.clone()),
        }
    }
}

impl JsonArg {
    /// Unquoted content, for display.
    pub fn text(&self) -> &str {
        match self {
            JsonArg::Str(s) => s,
            JsonArg::Word(w) => w,
        }
    }
}

impl From<&JsonArg> for Arg {
    fn from(a: &JsonArg) -> Arg {
        match a {
            JsonArg::Str(s) => Arg::Str(s.clone()),
            JsonArg::Word(w) => Arg::Word(w.clone()),
        }
    }
}

/// One configurable gesture slot, in stable display order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Slot {
    Tap,
    TapMore,
    SwipeMoreUp,
    SwipeMoreDown,
    HoldLeft,
    HoldRight,
    HoldUp,
    HoldDown,
    EdgeBottom,
    EdgeTop,
    EdgeLeft,
    EdgeRight,
    CornerTopLeft,
    CornerTopRight,
    CornerBottomLeft,
    CornerBottomRight,
}

impl Slot {
    pub const ALL: [Slot; 16] = [
        Slot::Tap,
        Slot::TapMore,
        Slot::SwipeMoreUp,
        Slot::SwipeMoreDown,
        Slot::HoldLeft,
        Slot::HoldRight,
        Slot::HoldUp,
        Slot::HoldDown,
        Slot::EdgeBottom,
        Slot::EdgeTop,
        Slot::EdgeLeft,
        Slot::EdgeRight,
        Slot::CornerTopLeft,
        Slot::CornerTopRight,
        Slot::CornerBottomLeft,
        Slot::CornerBottomRight,
    ];

    /// Key used in KDL parent nodes and in exported JSON.
    pub fn id(&self) -> &'static str {
        match self {
            Slot::Tap => "tap",
            Slot::TapMore => "tap-more",
            Slot::SwipeMoreUp => "swipe-more-up",
            Slot::SwipeMoreDown => "swipe-more-down",
            Slot::HoldLeft => "left",
            Slot::HoldRight => "right",
            Slot::HoldUp => "up",
            Slot::HoldDown => "down",
            Slot::EdgeBottom => "bottom",
            Slot::EdgeTop => "top",
            Slot::EdgeLeft => "left",
            Slot::EdgeRight => "right",
            Slot::CornerTopLeft => "top-left",
            Slot::CornerTopRight => "top-right",
            Slot::CornerBottomLeft => "bottom-left",
            Slot::CornerBottomRight => "bottom-right",
        }
    }

    pub fn friendly_name(&self) -> &'static str {
        match self {
            Slot::Tap => "base-finger tap",
            Slot::TapMore => "extra-finger tap",
            Slot::SwipeMoreUp => "extra-finger flick up",
            Slot::SwipeMoreDown => "extra-finger flick down",
            Slot::HoldLeft => "Hold + swipe left",
            Slot::HoldRight => "Hold + swipe right",
            Slot::HoldUp => "Hold + swipe up",
            Slot::HoldDown => "Hold + swipe down",
            Slot::EdgeBottom => "Bottom edge swipe",
            Slot::EdgeTop => "Top edge swipe",
            Slot::EdgeLeft => "Left edge swipe",
            Slot::EdgeRight => "Right edge swipe",
            Slot::CornerTopLeft => "Top-left corner swipe",
            Slot::CornerTopRight => "Top-right corner swipe",
            Slot::CornerBottomLeft => "Bottom-left corner swipe",
            Slot::CornerBottomRight => "Bottom-right corner swipe",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Slot::Tap => "Quick tap with the base finger count",
            Slot::TapMore => "Quick tap with one finger more than the base count",
            Slot::SwipeMoreUp => "Fast flick with one finger more than the base count, upward",
            Slot::SwipeMoreDown => "Fast flick with one finger more than the base count, downward",
            Slot::HoldLeft => "Touch down, hold ~400 ms, then swipe left",
            Slot::HoldRight => "Touch down, hold ~400 ms, then swipe right",
            Slot::HoldUp => "Touch down, hold ~400 ms, then swipe up",
            Slot::HoldDown => "Touch down, hold ~400 ms, then swipe down",
            Slot::EdgeBottom => "One finger swiping inward from the bottom edge",
            Slot::EdgeTop => "One finger swiping inward from the top edge",
            Slot::EdgeLeft => "One finger swiping inward from the left edge",
            Slot::EdgeRight => "One finger swiping inward from the right edge",
            Slot::CornerTopLeft => "One finger swiping diagonally in from the top-left corner",
            Slot::CornerTopRight => "One finger swiping diagonally in from the top-right corner",
            Slot::CornerBottomLeft => "One finger swiping diagonally in from the bottom-left corner",
            Slot::CornerBottomRight => "One finger swiping diagonally in from the bottom-right corner",
        }
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.friendly_name())
    }
}

/// A single parsed niri action node, e.g. `maximize-column` or
/// `spawn "foot" "-e" "htop"`.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ActionNode {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<JsonArg>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<(String, JsonArg)>,
}

impl ActionNode {
    pub fn from_kdl(n: &Node) -> ActionNode {
        ActionNode {
            name: n.name.clone(),
            args: n.args.iter().map(Into::into).collect(),
            props: n
                .props
                .iter()
                .map(|(k, v)| (k.clone(), JsonArg::from(v)))
                .collect(),
        }
    }

    pub fn to_kdl(&self) -> Node {
        Node {
            name: self.name.clone(),
            props: self
                .props
                .iter()
                .map(|(k, v)| (k.clone(), Arg::from(v)))
                .collect(),
            args: self.args.iter().map(Into::into).collect(),
            children: Vec::new(),
        }
    }

    /// Short human label, e.g. `spawn foot -e htop` or `toggle-overview`.
    pub fn label(&self) -> String {
        let mut s = self.name.clone();
        for a in &self.args {
            s.push(' ');
            s.push_str(a.text());
        }
        for (k, v) in &self.props {
            s.push_str(&format!(" {k}={}", v.text()));
        }
        s
    }
}

/// What a gesture slot does.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ActionValue {
    /// Unbound (node omitted; falls back to built-in behavior, if any).
    None,
    /// A single niri action node.
    Node(Box<ActionNode>),
    /// Something we can't cleanly model (multiple action nodes, unexpected
    /// shape). Kept verbatim so saving never destroys it.
    Raw {
        kdl: String,
    },
}

impl ActionValue {
    #[cfg(test)]
    pub fn label(&self) -> Option<String> {
        match self {
            ActionValue::None => None,
            ActionValue::Node(n) => Some(n.label()),
            ActionValue::Raw { .. } => Some("(custom KDL)".to_string()),
        }
    }

    pub fn simple(name: &str) -> ActionValue {
        ActionValue::Node(Box::new(ActionNode {
            name: name.to_string(),
            args: Vec::new(),
            props: Vec::new(),
        }))
    }

    pub fn spawn(command: &[String]) -> ActionValue {
        ActionValue::Node(Box::new(ActionNode {
            name: "spawn".to_string(),
            args: command.iter().map(|s| JsonArg::Str(s.clone())).collect(),
            props: Vec::new(),
        }))
    }

    pub fn spawn_sh(line: &str) -> ActionValue {
        ActionValue::Node(Box::new(ActionNode {
            name: "spawn-sh".to_string(),
            args: vec![JsonArg::Str(line.to_string())],
            props: Vec::new(),
        }))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum HorizontalSwipe {
    #[default]
    #[serde(rename = "move-view")]
    MoveView,
    #[serde(rename = "resize-column")]
    ResizeColumn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum ShowTouchPoints {
    #[default]
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "gestures")]
    Gestures,
    #[serde(rename = "all")]
    All,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Options {
    /// `off` inside touchscreen-swipe: disables the multi-finger stack.
    pub off: bool,
    /// `fingers N` (base finger count, 3 or 4; None = compiled default 3).
    pub fingers: Option<u8>,
    pub horizontal_swipe: HorizontalSwipe,
    pub show_touch_points: ShowTouchPoints,
    pub debug_log: bool,
}

/// Everything the app manages inside one `gestures {}` node.
///
/// Not serialized directly: hold/edge/corner slot ids repeat ("left" is both
/// a hold direction and an edge), which would collide as JSON keys. The
/// export module builds a DTO with unique keys instead.
#[derive(Clone, PartialEq, Debug)]
pub struct GestureModel {
    pub slots: BTreeMap<Slot, ActionValue>,
    pub options: Options,
    /// Unmodeled child nodes (hot-corners, dnd-edge-*, anything new),
    /// preserved structurally on save.
    pub passthrough: Vec<String>,
    /// Notes for the user gathered while parsing (legacy names migrated,
    /// invalid values reset). Cleared on every re-parse; shown as banners.
    pub warnings: Vec<String>,
}

impl Default for GestureModel {
    fn default() -> Self {
        GestureModel {
            slots: Slot::ALL
                .iter()
                .map(|s| (*s, ActionValue::None))
                .collect(),
            options: Options::default(),
            passthrough: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

impl GestureModel {
    pub fn get(&self, slot: Slot) -> &ActionValue {
        self.slots.get(&slot).unwrap_or(&ActionValue::None)
    }

    pub fn set(&mut self, slot: Slot, value: ActionValue) {
        self.slots.insert(slot, value);
    }

    /// Extract a GestureModel from a parsed `gestures` node.
    pub fn from_gestures_node(g: &Node) -> Result<GestureModel, String> {
        let mut model = GestureModel::default();
        for child in &g.children {
            match child.name.as_str() {
                "show-touch-points" => {
                    let v = child
                        .args
                        .first()
                        .map(|a| a.text().to_string())
                        .ok_or("show-touch-points needs a value")?;
                    model.options.show_touch_points = match v.as_str() {
                        "all" => ShowTouchPoints::All,
                        "gestures" => ShowTouchPoints::Gestures,
                        other => return Err(format!("unknown show-touch-points value {other:?}")),
                    };
                }
                "debug-log" => model.options.debug_log = true,
                "touchscreen-swipe" => {
                    Self::read_touchscreen_swipe(&mut model, child)?;
                }
                "touchscreen-edge-swipe" => {
                    Self::read_directional(&mut model, child, &[Slot::EdgeBottom, Slot::EdgeTop, Slot::EdgeLeft, Slot::EdgeRight])?;
                }
                "touchscreen-corner-swipe" => {
                    Self::read_directional(&mut model, child, &[Slot::CornerTopLeft, Slot::CornerTopRight, Slot::CornerBottomLeft, Slot::CornerBottomRight])?;
                }
                _ => model.passthrough.push(child.render(1)),
            }
        }
        Ok(model)
    }

    fn read_touchscreen_swipe(model: &mut GestureModel, ts: &Node) -> Result<(), String> {
        for child in &ts.children {
            match child.name.as_str() {
                "off" => model.options.off = true,
                "fingers" => {
                    let n: u8 = child
                        .args
                        .first()
                        .and_then(|a| a.text().parse().ok())
                        .ok_or("fingers needs a number")?;
                    if matches!(n, 3 | 4) {
                        model.options.fingers = Some(n);
                    } else {
                        model.options.fingers = None;
                        model.warnings.push(format!(
                            "fingers {n} is not valid (3 or 4); using the default 3 until you save"
                        ));
                    }
                }
                "horizontal-swipe" => {
                    let v = child
                        .args
                        .first()
                        .map(|a| a.text().to_string())
                        .ok_or("horizontal-swipe needs a value")?;
                    model.options.horizontal_swipe = match v.as_str() {
                        "move-view" => HorizontalSwipe::MoveView,
                        "resize-column" => HorizontalSwipe::ResizeColumn,
                        other => return Err(format!("unknown horizontal-swipe value {other:?}")),
                    };
                }
                "tap" => model.set(Slot::Tap, slot_action(child)),
                "tap-more" => model.set(Slot::TapMore, slot_action(child)),
                "swipe-more-up" => model.set(Slot::SwipeMoreUp, slot_action(child)),
                "swipe-more-down" => model.set(Slot::SwipeMoreDown, slot_action(child)),
                // Legacy names from release v26.04.18 and earlier: read as
                // their renamed slots (the new name wins if both appear) and
                // migrate away on save.
                "tap-4" => {
                    if matches!(model.get(Slot::TapMore), ActionValue::None) {
                        model.set(Slot::TapMore, slot_action(child));
                    }
                    model
                        .warnings
                        .push("tap-4 was renamed tap-more; saving writes the new name".to_string());
                }
                "swipe-4-up" => {
                    if matches!(model.get(Slot::SwipeMoreUp), ActionValue::None) {
                        model.set(Slot::SwipeMoreUp, slot_action(child));
                    }
                    model.warnings.push(
                        "swipe-4-up was renamed swipe-more-up; saving writes the new name"
                            .to_string(),
                    );
                }
                "swipe-4-down" => {
                    if matches!(model.get(Slot::SwipeMoreDown), ActionValue::None) {
                        model.set(Slot::SwipeMoreDown, slot_action(child));
                    }
                    model.warnings.push(
                        "swipe-4-down was renamed swipe-more-down; saving writes the new name"
                            .to_string(),
                    );
                }
                "hold" => {
                    for dir in &child.children {
                        let slot = match dir.name.as_str() {
                            "left" => Slot::HoldLeft,
                            "right" => Slot::HoldRight,
                            "up" => Slot::HoldUp,
                            "down" => Slot::HoldDown,
                            _ => {
                                model.passthrough.push(dir.render(2));
                                continue;
                            }
                        };
                        model.set(slot, slot_action(dir));
                    }
                }
                _ => model.passthrough.push(child.render(2)),
            }
        }
        Ok(())
    }

    fn read_directional(model: &mut GestureModel, parent: &Node, slots: &[Slot]) -> Result<(), String> {
        for child in &parent.children {
            let Some(slot) = slots.iter().find(|s| s.id() == child.name) else {
                model.passthrough.push(child.render(2));
                continue;
            };
            model.set(*slot, slot_action(child));
        }
        Ok(())
    }

    /// Render the managed `gestures {}` node (without file header comment).
    pub fn to_gestures_node(&self) -> Node {
        let mut g = Node::new("gestures");

        if self.options.show_touch_points != ShowTouchPoints::Off {
            let v = match self.options.show_touch_points {
                ShowTouchPoints::Gestures => "gestures",
                ShowTouchPoints::All => "all",
                ShowTouchPoints::Off => unreachable!(),
            };
            g.children
                .push(Node::new("show-touch-points").arg_str(v));
        }
        if self.options.debug_log {
            g.children.push(Node::new("debug-log"));
        }

        // Multi-finger block.
        let hold_slots = [
            Slot::HoldLeft,
            Slot::HoldRight,
            Slot::HoldUp,
            Slot::HoldDown,
        ];
        let mut ts = Node::new("touchscreen-swipe");
        if self.options.off {
            ts.children.push(Node::new("off"));
        }
        if let Some(f) = self.options.fingers {
            // u8 in niri's config: must be a bare number, not a string.
            let mut n = Node::new("fingers");
            n.args.push(kdl::Arg::Word(f.to_string()));
            ts.children.push(n);
        }
        if self.options.horizontal_swipe == HorizontalSwipe::ResizeColumn {
            ts.children
                .push(Node::new("horizontal-swipe").arg_str("resize-column"));
        }
        for (slot, parent) in [
            (Slot::Tap, "tap"),
            (Slot::TapMore, "tap-more"),
            (Slot::SwipeMoreDown, "swipe-more-down"),
            (Slot::SwipeMoreUp, "swipe-more-up"),
        ] {
            if let Some(node) = slot_node(self.get(slot)) {
                let mut n = Node::new(parent);
                n.children.push(node);
                ts.children.push(n);
            }
        }
        if hold_slots.iter().any(|s| !matches!(self.get(*s), ActionValue::None)) {
            let mut hold = Node::new("hold");
            for slot in hold_slots {
                if let Some(node) = slot_node(self.get(slot)) {
                    let mut d = Node::new(slot.id());
                    d.children.push(node);
                    hold.children.push(d);
                }
            }
            ts.children.push(hold);
        }
        // Always emit the block: it documents the feature even when empty.
        g.children.push(ts);

        // Edge and corner blocks: only when something is bound.
        let edge_slots = [Slot::EdgeBottom, Slot::EdgeTop, Slot::EdgeLeft, Slot::EdgeRight];
        if edge_slots.iter().any(|s| !matches!(self.get(*s), ActionValue::None)) {
            let mut n = Node::new("touchscreen-edge-swipe");
            for slot in edge_slots {
                if let Some(a) = slot_node(self.get(slot)) {
                    let mut d = Node::new(slot.id());
                    d.children.push(a);
                    n.children.push(d);
                }
            }
            g.children.push(n);
        }
        let corner_slots = [
            Slot::CornerTopLeft,
            Slot::CornerTopRight,
            Slot::CornerBottomLeft,
            Slot::CornerBottomRight,
        ];
        if corner_slots.iter().any(|s| !matches!(self.get(*s), ActionValue::None)) {
            let mut n = Node::new("touchscreen-corner-swipe");
            for slot in corner_slots {
                if let Some(a) = slot_node(self.get(slot)) {
                    let mut d = Node::new(slot.id());
                    d.children.push(a);
                    n.children.push(d);
                }
            }
            g.children.push(n);
        }

        // Unmodeled nodes, last (merge semantics make order between
        // differing nodes irrelevant). Passthrough strings are rendered node
        // text; re-parse to nest them structurally.
        for raw in &self.passthrough {
            if let Ok(doc) = kdl::parse_document(&format!("gestures {{\n{raw}\n}}")) {
                if let Some(gg) = kdl::find_top_level(&doc, "gestures") {
                    g.children.extend(gg.children.iter().cloned());
                }
            }
        }

        g
    }

    /// Full managed file text, header comment included.
    pub fn to_file_text(&self) -> String {
        let mut out = String::from(
            "// Managed by niri-tablet-easysetup. Edits here are overwritten when\n\
             // you save from the app; keep custom nodes out of it or import them\n\
             // back. niri reloads this file live (no re-login needed).\n",
        );
        out.push_str(&self.to_gestures_node().render(0));
        out.push('\n');
        out
    }

    /// Parse a whole gestures file (one top-level `gestures` node expected).
    /// Returns None when the file has no gestures node.
    pub fn parse_file_text(text: &str) -> Result<Option<GestureModel>, String> {
        let doc = kdl::parse_document(text).map_err(|e| e.to_string())?;
        let Some(g) = kdl::find_top_level(&doc, "gestures") else {
            return Ok(None);
        };
        Ok(Some(Self::from_gestures_node(g)?))
    }
}

/// Read one slot wrapper node (`tap { ... }`, `bottom { ... }`) into an
/// ActionValue. Anything unexpected becomes Raw passthrough.
fn slot_action(wrapper: &Node) -> ActionValue {
    if !wrapper.args.is_empty() || !wrapper.props.is_empty() {
        return ActionValue::Raw {
            kdl: wrapper.render(0),
        };
    }
    match wrapper.children.as_slice() {
        [] => ActionValue::None,
        [only] if only.children.is_empty() => ActionValue::Node(Box::new(ActionNode::from_kdl(only))),
        _ => ActionValue::Raw {
            kdl: wrapper.render(0),
        },
    }
}

/// Build the inner action node for writing, if the slot is set.
fn slot_node(value: &ActionValue) -> Option<Node> {
    match value {
        ActionValue::None => None,
        ActionValue::Node(n) => Some(n.to_kdl()),
        ActionValue::Raw { kdl } => {
            // The stored text is a rendered wrapper node; re-parse and take
            // its children so it lands inside the new wrapper correctly.
            match kdl::parse_document(kdl) {
                Ok(doc) => doc.first().map(|wrapper| wrapper.children.clone()).map(
                    |mut kids| {
                        if kids.len() == 1 {
                            kids.remove(0)
                        } else {
                            // Multiple children in one slot: keep them all so
                            // niri validate flags it instead of us dropping
                            // anything.
                            let mut n = Node::new("__easysetup_raw__");
                            n.children = kids;
                            n
                        }
                    },
                ),
                // Unparseable raw text: emit a loud placeholder that makes
                // `niri validate` refuse the save. Never drop silently.
                Err(_) => Some(Node::new("__easysetup_broken_raw__").arg_str(kdl)),
            }
        }
    }
}

/// The gesture scheme niri-tablet ships. Embedded straight from the repo's
/// config/gestures.kdl (single source of truth): first-run setup seeds a new
/// gestures file from it and "reset to defaults" re-imports it. Only the
/// `gestures` node is used; surrounding comments in that file are not.
pub const SHIPPED_DEFAULT: &str = include_str!("../../config/gestures.kdl");

pub fn shipped_default() -> GestureModel {
    GestureModel::parse_file_text(SHIPPED_DEFAULT)
        .expect("embedded shipped default must parse")
        .expect("embedded shipped default has a gestures node")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_default_parses_with_expected_slots() {
        let m = shipped_default();
        assert_eq!(m.get(Slot::Tap), &ActionValue::simple("maximize-column"));
        assert_eq!(m.get(Slot::TapMore), &ActionValue::simple("toggle-overview"));
        assert_eq!(
            m.get(Slot::SwipeMoreDown),
            &ActionValue::simple("close-window")
        );
        assert_eq!(
            m.get(Slot::SwipeMoreUp),
            &ActionValue::simple("fullscreen-window")
        );
        assert_eq!(m.get(Slot::EdgeBottom), &ActionValue::spawn_sh("~/.local/bin/niri-osk.sh"));
        assert_eq!(m.get(Slot::CornerTopLeft), &ActionValue::None);
        assert_eq!(m.options.fingers, Some(3));
        assert_eq!(m.options.horizontal_swipe, HorizontalSwipe::ResizeColumn);
        assert_eq!(m.options.show_touch_points, ShowTouchPoints::Gestures);
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    }

    #[test]
    fn round_trip_shipped_default() {
        let m = shipped_default();
        let text = m.to_file_text();
        let back = GestureModel::parse_file_text(&text)
            .unwrap()
            .expect("gestures node");
        assert_eq!(m.slots, back.slots);
        assert_eq!(m.options, back.options);
        assert!(back.passthrough.is_empty());
        // Render is stable across round trips.
        assert_eq!(text, back.to_file_text());
    }

    #[test]
    fn preserves_unmodeled_nodes_and_local_edits() {
        // Shape of the maintainer's real file (pre-rename): local spawn-sh
        // edits under the legacy names plus an unmodeled hot-corners node
        // that must survive a save while the names migrate.
        let src = r#"gestures {
    show-touch-points "gestures"
    touchscreen-swipe {
        fingers 3
        horizontal-swipe "resize-column"
        tap          { maximize-column; }
        tap-4        { spawn-sh "noctalia msg panel-toggle launcher"; }
        swipe-4-down { close-window; }
        swipe-4-up   { spawn-sh "~/.local/bin/niri-osk.sh"; }
        hold {
            left { move-column-left; }
            right { move-column-right; }
            up { move-window-to-workspace-up; }
            down { move-window-to-workspace-down; }
        }
    }
    touchscreen-edge-swipe {
        bottom { spawn-sh "~/.local/bin/niri-osk.sh"; }
        top { toggle-overview; }
        left { focus-column-left; }
        right { focus-column-right; }
    }
    hot-corners {
        off
        top-left
    }
}
"#;
        let m = GestureModel::parse_file_text(src)
            .unwrap()
            .expect("gestures node");
        assert_eq!(
            m.get(Slot::TapMore).label().as_deref(),
            Some("spawn-sh noctalia msg panel-toggle launcher")
        );
        assert_eq!(m.passthrough.len(), 1);
        // Legacy names migrate on save; they never reach passthrough.
        let text = m.to_file_text();
        assert!(text.contains("hot-corners"), "passthrough node lost:\n{text}");
        assert!(text.contains("tap-more"), "{text}");
        assert!(!text.contains("tap-4"), "legacy name survived:\n{text}");
        let back = GestureModel::parse_file_text(&text).unwrap().unwrap();
        assert_eq!(m.slots, back.slots);
        assert_eq!(m.passthrough, back.passthrough);
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    }

    #[test]
    fn legacy_four_finger_names_migrate() {
        let src = "gestures {\n    touchscreen-swipe {\n        tap-4 { toggle-overview; }\n        swipe-4-up { close-window; }\n        swipe-4-down { fullscreen-window; }\n    }\n}\n";
        let m = GestureModel::parse_file_text(src).unwrap().unwrap();
        assert_eq!(m.get(Slot::TapMore), &ActionValue::simple("toggle-overview"));
        assert_eq!(m.get(Slot::SwipeMoreUp), &ActionValue::simple("close-window"));
        assert_eq!(m.get(Slot::SwipeMoreDown), &ActionValue::simple("fullscreen-window"));
        assert_eq!(m.warnings.len(), 3);
        assert!(m.passthrough.is_empty());
    }

    #[test]
    fn new_name_wins_over_legacy() {
        let src = "gestures {\n    touchscreen-swipe {\n        tap-4 { toggle-overview; }\n        tap-more { close-window; }\n    }\n}\n";
        let m = GestureModel::parse_file_text(src).unwrap().unwrap();
        assert_eq!(m.get(Slot::TapMore), &ActionValue::simple("close-window"));
    }

    #[test]
    fn out_of_range_fingers_resets() {
        let src = "gestures {\n    touchscreen-swipe {\n        fingers 5\n    }\n}\n";
        let m = GestureModel::parse_file_text(src).unwrap().unwrap();
        assert_eq!(m.options.fingers, None);
        assert_eq!(m.warnings.len(), 1);
    }

    #[test]
    fn spawn_with_args_round_trips() {
        let src = r#"gestures {
    touchscreen-corner-swipe {
        bottom-left { spawn "wofi" "--show" "drun"; }
    }
}
"#;
        let m = GestureModel::parse_file_text(src).unwrap().unwrap();
        assert_eq!(
            m.get(Slot::CornerBottomLeft),
            &ActionValue::spawn(&[
                "wofi".to_string(),
                "--show".to_string(),
                "drun".to_string()
            ])
        );
        let text = m.to_file_text();
        assert!(text.contains(r#"spawn "wofi" "--show" "drun";"#), "{text}");
    }

    #[test]
    fn empty_and_raw_slots() {
        let src = "gestures {\n    touchscreen-swipe {\n        tap { maximize-column; quit skip-confirmation=true; }\n    }\n}\n";
        let m = GestureModel::parse_file_text(src).unwrap().unwrap();
        assert!(matches!(m.get(Slot::Tap), ActionValue::Raw { .. }));
        // Round trip keeps it verbatim inside the slot.
        let text = m.to_file_text();
        assert!(text.contains("quit skip-confirmation=true"), "{text}");
    }
}
