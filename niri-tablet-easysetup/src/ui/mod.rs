//! GTK UI: app state and entry point.

use adw::prelude::*;

mod picker;
mod window;

use crate::discovery::Discovery;
use crate::model::GestureModel;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub struct AppState {
    pub discovery: Discovery,
    pub model: GestureModel,
    /// Snapshot of the last applied/saved state; model != saved means dirty.
    pub saved: GestureModel,
    pub niri: Option<PathBuf>,
    /// Does the installed niri understand the gestures config?
    pub niri_supports_gestures: bool,
    pub ipc: bool,
    /// Warnings collected at load (parse failures and the like).
    pub load_warnings: Vec<String>,
    /// Keep-alives for native file dialogs (they must not be dropped while
    /// visible).
    pub native_dialogs: RefCell<Vec<gtk::FileDialog>>,
}

pub type SharedState = Rc<RefCell<AppState>>;

impl AppState {
    pub fn dirty(&self) -> bool {
        self.model != self.saved
    }
}

pub fn run() {
    let app = adw::Application::builder()
        .application_id("com.github.ggezus.NiriTabletEasySetup")
        .build();
    app.connect_activate(|app| {
        window::build(app);
    });
    app.run();
}

/// Run blocking work off the UI thread and hand the result back to the main
/// loop. `on_done` runs on the main context (no Send bound needed).
pub fn blocking<T, F, G>(work: F, on_done: G)
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
    G: FnOnce(T) + 'static,
{
    let handle = gtk::gio::spawn_blocking(work);
    gtk::glib::MainContext::default().spawn_local(async move {
        match handle.await {
            Ok(t) => on_done(t),
            Err(_) => eprintln!("easysetup: background task failed"),
        }
    });
}

/// Human label for what a slot currently does.
pub fn action_display(value: &crate::model::ActionValue) -> String {
    use crate::model::ActionValue;
    match value {
        ActionValue::None => "Unbound".to_string(),
        ActionValue::Raw { .. } => "Custom KDL".to_string(),
        ActionValue::Node(n) => {
            if let Some(entry) = crate::catalog::find(&n.name) {
                entry.label.to_string()
            } else {
                match n.name.as_str() {
                    "spawn" => format!("Run: {}", n.args.iter().map(|a| a.text()).collect::<Vec<_>>().join(" ")),
                    "spawn-sh" => format!("Run: {}", n.args.first().map(|a| a.text().to_string()).unwrap_or_default()),
                    _ => n.label(),
                }
            }
        }
    }
}
