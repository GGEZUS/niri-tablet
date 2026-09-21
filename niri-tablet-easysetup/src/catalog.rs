//! A curated, friendly list of niri actions worth binding to gestures.
//! Anything not here is still fully supported via "Custom KDL".

use crate::model::ActionValue;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Windows,
    Columns,
    Workspaces,
    Monitors,
    Overview,
    Screenshots,
    System,
}

impl Category {
    pub fn label(&self) -> &'static str {
        match self {
            Category::Windows => "Windows",
            Category::Columns => "Columns",
            Category::Workspaces => "Workspaces",
            Category::Monitors => "Monitors",
            Category::Overview => "Overview & layout",
            Category::Screenshots => "Screenshots",
            Category::System => "System",
        }
    }
}

/// What argument editor (if any) an entry needs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Args {
    None,
    /// Size change like `+10%`, `50%`, `100` with quick presets.
    SizeChange,
    /// Workspace reference: a number (index) or a name.
    WorkspaceRef,
    /// One of a fixed set of words.
    Fixed(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    /// KDL node name; also the IPC action name.
    pub name: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub args: Args,
    /// None = cannot be safely test-run via `niri msg`.
    pub test_note: Option<&'static str>,
}

const YES: Option<&'static str> = None;

macro_rules! e {
    ($name:literal, $label:literal, $cat:ident, $args:expr) => {
        Entry { name: $name, label: $label, category: Category::$cat, args: $args, test_note: YES }
    };
    ($name:literal, $label:literal, $cat:ident, $args:expr, no_test: $why:literal) => {
        Entry { name: $name, label: $label, category: Category::$cat, args: $args, test_note: Some($why) }
    };
}

pub static ENTRIES: &[Entry] = &[
    // Windows: manage
    e!("close-window", "Close window", Windows, Args::None),
    e!("fullscreen-window", "Fullscreen on/off", Windows, Args::None),
    e!("toggle-windowed-fullscreen", "Windowed fullscreen on/off", Windows, Args::None),
    e!("maximize-column", "Maximize or restore the focused column", Windows, Args::None),
    e!("maximize-window-to-edges", "Maximize a floating window to screen edges", Windows, Args::None),
    e!("toggle-window-floating", "Float or tile the focused window", Windows, Args::None),
    e!("move-window-to-floating", "Make the window floating", Windows, Args::None),
    e!("move-window-to-tiling", "Make the window tiled", Windows, Args::None),
    e!("swap-window-left", "Swap window with the left neighbor", Windows, Args::None),
    e!("swap-window-right", "Swap window with the right neighbor", Windows, Args::None),
    e!("center-window", "Center a floating window", Windows, Args::None),
    e!("center-column", "Center the focused column", Windows, Args::None),
    // Windows: focus
    e!("focus-window-up", "Focus window above", Windows, Args::None),
    e!("focus-window-down", "Focus window below", Windows, Args::None),
    e!("focus-window-or-workspace-up", "Focus window above, else workspace up", Windows, Args::None),
    e!("focus-window-or-workspace-down", "Focus window below, else workspace down", Windows, Args::None),
    e!("focus-window-top", "Focus the top window", Windows, Args::None),
    e!("focus-window-bottom", "Focus the bottom window", Windows, Args::None),
    e!("focus-window-previous", "Focus the previously focused window", Windows, Args::None),
    // Windows: size
    e!("set-window-width", "Set window width", Windows, Args::SizeChange),
    e!("set-window-height", "Set window height", Windows, Args::SizeChange),
    // Columns
    e!("focus-column-left", "Focus the column on the left", Columns, Args::None),
    e!("focus-column-right", "Focus the column on the right", Columns, Args::None),
    e!("focus-column-first", "Focus the first column", Columns, Args::None),
    e!("focus-column-last", "Focus the last column", Columns, Args::None),
    e!("focus-column-right-or-first", "Focus right, wrap to first column", Columns, Args::None),
    e!("focus-column-left-or-last", "Focus left, wrap to last column", Columns, Args::None),
    e!("move-column-left", "Move the column left", Columns, Args::None),
    e!("move-column-right", "Move the column right", Columns, Args::None),
    e!("move-column-to-first", "Move the column to the first position", Columns, Args::None),
    e!("move-column-to-last", "Move the column to the last position", Columns, Args::None),
    e!("consume-or-expel-window-left", "Fold into the left column, or unfold", Columns, Args::None),
    e!("consume-or-expel-window-right", "Fold into the right column, or unfold", Columns, Args::None),
    e!("consume-window-into-column", "Stack the window into its column", Columns, Args::None),
    e!("expel-window-from-column", "Unstack the window from its column", Columns, Args::None),
    e!("toggle-column-tabbed-display", "Tabs on/off in the focused column", Columns, Args::None),
    e!("switch-preset-column-width", "Cycle preset column widths", Columns, Args::None),
    e!("expand-column-to-available-width", "Expand column to fill free space", Columns, Args::None),
    e!("set-column-width", "Set column width", Columns, Args::SizeChange),
    // Workspaces
    e!("focus-workspace-up", "Workspace above", Workspaces, Args::None),
    e!("focus-workspace-down", "Workspace below", Workspaces, Args::None),
    e!("focus-workspace-previous", "Previously used workspace", Workspaces, Args::None),
    e!("focus-workspace", "Go to a workspace", Workspaces, Args::WorkspaceRef),
    e!("move-window-to-workspace-up", "Move window to the workspace above", Workspaces, Args::None),
    e!("move-window-to-workspace-down", "Move window to the workspace below", Workspaces, Args::None),
    e!("move-column-to-workspace-up", "Move column to the workspace above", Workspaces, Args::None),
    e!("move-column-to-workspace-down", "Move column to the workspace below", Workspaces, Args::None),
    e!("move-window-to-workspace", "Move window to a workspace", Workspaces, Args::WorkspaceRef),
    e!("move-workspace-up", "Reorder workspace up", Workspaces, Args::None),
    e!("move-workspace-down", "Reorder workspace down", Workspaces, Args::None),
    e!("set-workspace-name", "Name this workspace", Workspaces, Args::WorkspaceRef),
    // Monitors
    e!("focus-monitor-left", "Focus the monitor on the left", Monitors, Args::None),
    e!("focus-monitor-right", "Focus the monitor on the right", Monitors, Args::None),
    e!("focus-monitor-up", "Focus the monitor above", Monitors, Args::None),
    e!("focus-monitor-down", "Focus the monitor below", Monitors, Args::None),
    e!("focus-monitor-previous", "Focus the previous monitor", Monitors, Args::None),
    e!("focus-monitor-next", "Focus the next monitor", Monitors, Args::None),
    e!("move-window-to-monitor-left", "Move window to the left monitor", Monitors, Args::None),
    e!("move-window-to-monitor-right", "Move window to the right monitor", Monitors, Args::None),
    e!("move-column-to-monitor-left", "Move column to the left monitor", Monitors, Args::None),
    e!("move-column-to-monitor-right", "Move column to the right monitor", Monitors, Args::None),
    e!("power-off-monitors", "Turn monitors off (DPMS)", Monitors, Args::None),
    e!("power-on-monitors", "Turn monitors on (DPMS)", Monitors, Args::None),
    // Overview & layout
    e!("toggle-overview", "Open or close the overview", Overview, Args::None),
    e!("open-overview", "Open the overview", Overview, Args::None),
    e!("close-overview", "Close the overview", Overview, Args::None),
    e!("show-hotkey-overlay", "Show the keyboard shortcuts overlay", Overview, Args::None),
    e!("switch-layout", "Switch the layout", Overview, Args::Fixed(&["next", "prev"])),
    e!("switch-focus-between-floating-and-tiling", "Jump between floating and tiling", Overview, Args::None),
    e!("focus-floating", "Focus a floating window", Overview, Args::None),
    e!("focus-tiling", "Focus a tiled window", Overview, Args::None),
    // Screenshots
    e!("screenshot", "Screenshot (interactive)", Screenshots, Args::None),
    e!("screenshot-screen", "Screenshot this screen", Screenshots, Args::None),
    e!("screenshot-window", "Screenshot the focused window", Screenshots, Args::None),
    e!("do-screen-transition", "Play a screen transition", Screenshots, Args::None),
    // System
    e!("quit", "Quit niri (ends the session)", System, Args::None,
        no_test: "would exit the running niri session"),
    e!("suspend", "Suspend the system", System, Args::None,
        no_test: "suspend has no IPC action to test with"),
];

pub fn find(name: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.name == name)
}

/// Build the ActionValue for a catalog entry once its argument (if any) is
/// chosen. `arg` is raw user text; empty means default where sensible.
pub fn to_action(entry: &Entry, arg: Option<&str>) -> ActionValue {
    let arg = arg.filter(|s| !s.trim().is_empty());
    match entry.args {
        Args::None => ActionValue::simple(entry.name),
        _ => {
            let text = arg.unwrap_or(match entry.args {
                Args::SizeChange => "+10%",
                Args::WorkspaceRef => "2",
                Args::Fixed(words) => words[0],
                Args::None => unreachable!(),
            });
            ActionValue::Node(Box::new(crate::model::ActionNode {
                name: entry.name.to_string(),
                args: vec![crate::model::JsonArg::from(&crate::kdl::Arg::from_user_text(text))],
                props: Vec::new(),
            }))
        }
    }
}

/// Words for `niri msg action ...` to fire this value on the running
/// compositor. None when it can't (or shouldn't) be tested.
pub fn msg_words(value: &ActionValue) -> Option<Vec<String>> {
    let (name, args): (String, Vec<crate::kdl::Arg>) = match value {
        ActionValue::None | ActionValue::Raw { .. } => return None,
        ActionValue::Node(n) => {
            if matches!(n.name.as_str(), "quit" | "suspend") {
                return None;
            }
            (
                n.name.clone(),
                n.args.iter().map(|a| crate::kdl::Arg::from(a)).collect(),
            )
        }
    };
    let mut words = vec![name];
    match words[0].as_str() {
        "spawn" | "spawn-sh" => {
            // clap `last = true`: everything after `--`.
            words.push("--".to_string());
        }
        _ => {}
    }
    words.extend(args.iter().map(|a| a.text().to_string()));
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kdl::parse_document;

    #[test]
    fn names_are_unique() {
        let mut names: Vec<&str> = ENTRIES.iter().map(|e| e.name).collect();
        names.sort_unstable();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate catalog names");
    }

    #[test]
    fn fixed_args_all_have_words() {
        for e in ENTRIES {
            if let Args::Fixed(words) = e.args {
                assert!(!words.is_empty(), "{} has empty fixed args", e.name);
            }
        }
    }

    #[test]
    fn every_action_renders_as_one_valid_node() {
        for e in ENTRIES {
            let arg = match e.args {
                Args::SizeChange => Some("+10%"),
                Args::WorkspaceRef => Some("2"),
                Args::Fixed(w) => Some(w[0]),
                Args::None => None,
            };
            let v = to_action(e, arg);
            let model_v = crate::model::ActionValue::simple("x");
            let _ = model_v;
            // Render through a gestures file and parse it back.
            let text = format!(
                "gestures {{\n    touchscreen-swipe {{\n        tap {{ {} }}\n    }}\n}}\n",
                render_node_text(&v)
            );
            let doc = parse_document(&text).expect(e.name);
            let tap = doc[0].children[0].children[0].children[0].clone();
            assert_eq!(tap.name, e.name, "round trip failed for {}", e.name);
        }
    }

    fn render_node_text(v: &ActionValue) -> String {
        match v {
            ActionValue::Node(n) => n.to_kdl().render(0),
            _ => panic!("catalog entries are always nodes"),
        }
    }

    #[test]
    fn msg_words_for_spawn_uses_dash_dash() {
        let v = ActionValue::spawn(&["foot".into(), "-e".into(), "htop".into()]);
        assert_eq!(
            msg_words(&v).unwrap(),
            vec!["spawn".to_string(), "--".into(), "foot".into(), "-e".into(), "htop".into()]
        );
    }
}
