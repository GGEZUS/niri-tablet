//! The main window: gesture rows, options, banners, apply flow.

use super::{action_display, AppState, SharedState};
use crate::discovery::Source;
use crate::model::{HorizontalSwipe, ShowTouchPoints, Slot};
use crate::store::{self, Saved};
use crate::{catalog, niri};
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::pango::EllipsizeMode;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Widget handles the refresh path needs; shared with every closure.
pub struct Ui {
    pub win: adw::ApplicationWindow,
    pub rows: HashMap<Slot, (adw::ActionRow, gtk::Label, gtk::Button)>,
    pub banner_box: gtk::Box,
    pub revealer: gtk::Revealer,
    pub apply_btn: gtk::Button,
    pub title: adw::WindowTitle,
    pub toasts: adw::ToastOverlay,
    /// The two multi-finger groups; titles/descriptions track the finger count
    /// (the second group is always one finger more than the first).
    pub base_group: adw::PreferencesGroup,
    pub more_group: adw::PreferencesGroup,
}

pub type SharedUi = Rc<Ui>;

pub fn build(app: &adw::Application) {
    let state: SharedState = Rc::new(RefCell::new(init_state()));
    let win = adw::ApplicationWindow::builder()
        .application(app)
        .title("Gestures")
        .default_width(480)
        .default_height(720)
        .build();

    let toolbar = adw::ToolbarView::new();

    let title = adw::WindowTitle::new("Gestures", "");
    let menu_btn = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .primary(true)
        .menu_model(&main_menu())
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&title));
    header.pack_end(&menu_btn);
    toolbar.add_top_bar(&header);

    let banner_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(6);
    content.set_margin_bottom(12);
    content.set_margin_start(6);
    content.set_margin_end(6);

    let mut rows: HashMap<Slot, (adw::ActionRow, gtk::Label, gtk::Button)> = HashMap::new();
    let mut controls = OptionControls::default();
    let mut groups = GroupHandles::default();
    build_gesture_groups(&content, &state, &mut rows, &mut controls, &mut groups);
    build_options(&content, &state, &mut controls);

    let clamp = adw::Clamp::new();
    clamp.set_child(Some(&content));
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&clamp));
    scrolled.set_vexpand(true);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&banner_box);
    column.append(&scrolled);

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&column));
    toolbar.set_content(Some(&toasts));

    let revert_btn = gtk::Button::builder()
        .label("Revert")
        .css_classes(["flat"])
        .build();
    let apply_btn = gtk::Button::builder()
        .label("Apply")
        .css_classes(["suggested-action"])
        .build();
    let apply_bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    apply_bar.set_margin_start(12);
    apply_bar.set_margin_end(12);
    apply_bar.set_margin_top(6);
    apply_bar.set_margin_bottom(6);
    apply_bar.append(&revert_btn);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    apply_bar.append(&spacer);
    apply_bar.append(&apply_btn);
    let revealer = gtk::Revealer::new();
    revealer.set_reveal_child(false);
    revealer.set_child(Some(&apply_bar));
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
    toolbar.add_bottom_bar(&revealer);

    win.set_content(Some(&toolbar));

    let ui: SharedUi = Rc::new(Ui {
        win: win.clone(),
        rows,
        banner_box,
        revealer,
        apply_btn: apply_btn.clone(),
        title,
        toasts,
        base_group: groups.base.expect("base group built"),
        more_group: groups.more.expect("more group built"),
    });
    // Wire row interactions now that the shared Ui exists.
    let slots: Vec<Slot> = ui.rows.keys().copied().collect();
    for slot in slots {
        let (_, _, test_btn) = &ui.rows[&slot];
        test_btn.connect_clicked({
            let state = state.clone();
            let ui = ui.clone();
            move |_| fire_test(&ui, &state, slot)
        });
        let (row, _, _) = &ui.rows[&slot];
        row.connect_activated({
            let state = state.clone();
            let ui = ui.clone();
            let row = row.clone();
            move |_| {
                if let Some(root) = row.root() {
                    if let Ok(win) = root.downcast::<adw::ApplicationWindow>() {
                        super::picker::open(&win, &state, &ui, slot);
                    }
                }
            }
        });
    }
    wire_options(&state, &ui, &controls);
    refresh(&state, &ui);

    apply_btn.connect_clicked({
        let state = state.clone();
        let ui = ui.clone();
        move |_| apply_and_finish(&ui, &state, None)
    });
    revert_btn.connect_clicked({
        let state = state.clone();
        let ui = ui.clone();
        move |_| {
            let saved = state.borrow().saved.clone();
            state.borrow_mut().model = saved;
            refresh(&state, &ui);
        }
    });

    build_menu_actions(&win, &menu_btn, &state, &ui);

    win.connect_close_request({
        let state = state.clone();
        let ui = ui.clone();
        move |win| {
            if !state.borrow().dirty() {
                return glib::Propagation::Proceed;
            }
            let dialog = adw::AlertDialog::new(
                Some("Unsaved changes"),
                Some("Apply the new gesture scheme before closing?"),
            );
            dialog.add_responses(&[("cancel", "Cancel"), ("discard", "Discard"), ("apply", "Apply")]);
            dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);
            dialog.connect_response(Some("apply"), {
                let state = state.clone();
                let ui = ui.clone();
                let win = win.clone();
                move |_, _| {
                    apply_and_finish(
                        &ui,
                        &state,
                        Some(Box::new({
                            let win = win.clone();
                            move || win.close()
                        })),
                    );
                }
            });
            dialog.connect_response(Some("discard"), {
                let state = state.clone();
                let ui = ui.clone();
                let win = win.clone();
                move |_, _| {
                    let saved = state.borrow().saved.clone();
                    state.borrow_mut().model = saved;
                    refresh(&state, &ui);
                    win.close();
                }
            });
            dialog.present(Some(win));
            glib::Propagation::Stop
        }
    });

    win.present();
}

pub fn refresh(state: &SharedState, ui: &SharedUi) {
    {
        let s = state.borrow();
        for (slot, (_, label, _)) in ui.rows.iter() {
            let text = action_display(s.model.get(*slot));
            label.set_text(&text);
            let unbound = matches!(s.model.get(*slot), crate::model::ActionValue::None);
            label.set_css_classes(if unbound {
                &["dim-label"]
            } else {
                &["dim-label", "heading"]
            });
        }
        let dirty = s.dirty();
        ui.revealer.set_reveal_child(dirty);
        ui.apply_btn.set_sensitive(dirty);
        // The multi-finger groups are named after the finger count so the
        // labels never lie: the second tier always sits at one finger more
        // than `fingers` (niri dispatches tap-more/swipe-more-* there).
        let n = s.model.options.fingers.unwrap_or(3);
        ui.base_group.set_title(&format!("{n}-finger gestures"));
        ui.base_group.set_description(Some(
            "Tap, hold-swipes, and the two animated drags",
        ));
        ui.more_group.set_title(&format!("{}-finger gestures", n + 1));
        ui.more_group.set_description(Some(&format!(
            "Tap and flicks at one finger more than the base; they take priority over the {n}-finger gestures. Leave a bind unset to fall back to the {n}-finger tap or the animated drags"
        )));
        let subtitle = match &s.discovery.source {
            Source::ManagedFile(p) | Source::ManagedFileAmbiguous(p, _) => p.display().to_string(),
            Source::Inline(p) => format!("inline in {}", p.display()),
            Source::None => "not set up yet".to_string(),
        };
        ui.title.set_subtitle(&subtitle);
    }
    refresh_banners(state, ui);
}

fn init_state() -> AppState {
    let loaded = store::load();
    let niri_bin = niri::find_niri();
    let (supports, _) = match &niri_bin {
        Some(p) => niri::gestures_supported(p),
        None => (false, String::new()),
    };
    AppState {
        discovery: loaded.discovery,
        saved: loaded.model.clone(),
        model: loaded.model,
        niri_supports_gestures: supports,
        ipc: niri::ipc_available(),
        niri: niri_bin,
        load_warnings: loaded.warnings,
        native_dialogs: RefCell::new(Vec::new()),
    }
}

fn main_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("Export scheme…"), Some("win.export"));
    menu.append(Some("Import scheme…"), Some("win.import"));
    menu.append(Some("Reset to shipped defaults"), Some("win.reset"));
    let about = gio::Menu::new();
    about.append(Some("About"), Some("win.about"));
    menu.append_section(None, &about);
    menu
}

fn build_menu_actions(win: &adw::ApplicationWindow, menu_btn: &gtk::MenuButton, state: &SharedState, ui: &SharedUi) {
    let actions = gio::SimpleActionGroup::new();

    let export = gio::SimpleAction::new("export", None);
    export.connect_activate({
        let win = win.clone();
        let state = state.clone();
        let ui = ui.clone();
        move |_, _| export_scheme(&win, &state, &ui)
    });

    let import = gio::SimpleAction::new("import", None);
    import.connect_activate({
        let win = win.clone();
        let state = state.clone();
        let ui = ui.clone();
        move |_, _| import_scheme(&win, &state, &ui)
    });

    let reset = gio::SimpleAction::new("reset", None);
    reset.connect_activate({
        let state = state.clone();
        let ui = ui.clone();
        move |_, _| {
            state.borrow_mut().model = crate::model::shipped_default();
            refresh(&state, &ui);
            toast(&ui, "Shipped defaults loaded; Apply to write them");
        }
    });

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(move |_, _| {
        let d = adw::AboutWindow::builder()
            .application_name("niri-tablet-easysetup")
            .version(env!("CARGO_PKG_VERSION"))
            .comments("Friendly configurator for niri-tablet touchscreen gestures")
            .website("https://github.com/GGEZUS/niri-tablet")
            .license_type(gtk::License::Gpl30)
            .build();
        d.present();
    });

    actions.add_action(&export);
    actions.add_action(&import);
    actions.add_action(&reset);
    actions.add_action(&about);
    menu_btn
        .upcast_ref::<gtk::Widget>()
        .insert_action_group("win", Some(&actions));
}

fn export_scheme(win: &adw::ApplicationWindow, state: &SharedState, ui: &SharedUi) {
    let dialog = gtk::FileDialog::builder()
        .title("Export gesture scheme")
        .initial_name("niri-gestures.json")
        .build();
    let state = state.clone();
    let ui = ui.clone();
    // Keep a reference for as long as the dialog may be running.
    state.borrow_mut().native_dialogs.borrow_mut().push(dialog.clone());
    dialog.save(Some(win), None::<&gio::Cancellable>, move |result| {
        if let Ok(file) = result {
            let json = crate::export::scheme_to_json(&state.borrow().model);
            match std::fs::write(file.path().unwrap_or_default(), json) {
                Ok(()) => toast(&ui, "Scheme exported"),
                Err(e) => toast(&ui, &format!("Export failed: {e}")),
            }
        }
    });
}

fn import_scheme(win: &adw::ApplicationWindow, state: &SharedState, ui: &SharedUi) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Gesture schemes"));
    filter.add_pattern("*.json");
    filter.add_pattern("*.kdl");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title("Import gesture scheme")
        .filters(&filters)
        .build();
    let state = state.clone();
    let ui2 = ui.clone();
    state.borrow_mut().native_dialogs.borrow_mut().push(dialog.clone());
    dialog.open(Some(win), None::<&gio::Cancellable>, move |result| {
        let Ok(file) = result else { return };
        let Some(path) = file.path() else { return };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                toast(&ui2, &format!("Import failed: {e}"));
                return;
            }
        };
        let imported = if path.extension().and_then(|e| e.to_str()) == Some("kdl") {
            crate::export::from_kdl_text(&text).map(|m| (m, Vec::new()))
        } else {
            crate::export::from_scheme_json(&text).map(|i| (i.model, i.ignored_keys))
        };
        match imported {
            Ok((model, ignored)) => {
                state.borrow_mut().model = model;
                refresh(&state, &ui2);
                if ignored.is_empty() {
                    toast(&ui2, "Scheme loaded; Apply to write it");
                } else {
                    toast(
                        &ui2,
                        &format!("Loaded (ignored unknown keys: {})", ignored.join(", ")),
                    );
                }
            }
            Err(e) => toast(&ui2, &format!("Import failed: {e}")),
        }
    });
}

/// How the running compositor answered after an Apply.
enum Confirmation {
    /// niri reloaded the config and reports it valid.
    Confirmed,
    /// niri tried to reload and rejected the config (keeps the old one).
    Rejected,
    /// No event stream; the file chain passes `niri validate` from disk.
    ValidatedDisk,
    /// No event stream and no niri binary.
    NoNiri,
    /// Something went wrong getting a verdict.
    Unknown(String),
}

pub fn apply_and_finish(ui: &SharedUi, state: &SharedState, on_done: Option<Box<dyn Fn() + 'static>>) {
    let d = state.borrow().discovery.clone();
    let m = state.borrow().model.clone();
    let niri_path = state.borrow().niri.clone();
    let state = state.clone();
    let ui = ui.clone();
    let m_work = m.clone();
    let d_work = d.clone();
    super::blocking(
        move || {
            // Subscribe before writing so the reload event can't be missed.
            let watch = niri_path.as_ref().and_then(|p| niri::event_stream(p));
            if let Some(w) = watch.as_ref() {
                // Discard the connect-time event (the last load attempt).
                let _ = w.wait_config_loaded(std::time::Duration::from_secs(2));
            }
            let result = store::save(&d_work, &m_work);

            let disk_fallback = |confirmation_needed: bool| -> Confirmation {
                if !confirmation_needed {
                    return Confirmation::NoNiri;
                }
                let Some(p) = &niri_path else {
                    return Confirmation::NoNiri;
                };
                match d_work.config_path.as_ref().and_then(|c| niri::validate(p, c).ok()) {
                    Some(()) => Confirmation::ValidatedDisk,
                    None => Confirmation::Unknown("event stream silent; validate unavailable".into()),
                }
            };

            let confirmation = match (&result, watch.as_ref()) {
                (Ok(_), Some(w)) => match w.wait_config_loaded(std::time::Duration::from_millis(3500)) {
                    Some(false) => Confirmation::Confirmed,
                    Some(true) => Confirmation::Rejected,
                    None => disk_fallback(true),
                },
                (Ok(_), None) => disk_fallback(niri_path.is_some()),
                (Err(_), _) => Confirmation::NoNiri, // unused; the error path shows a dialog
            };
            (result, confirmation)
        },
        move |(result, confirmation)| match result {
            Ok(Saved::Written { path }) => {
                {
                    let mut s = state.borrow_mut();
                    s.saved = m.clone();
                    // Source may have changed (first run / extraction).
                    s.discovery = crate::discovery::discover();
                }
                match confirmation {
                    Confirmation::Confirmed => {
                        toast(&ui, "Applied and verified: niri reloaded the config and it is valid");
                    }
                    Confirmation::Rejected => {
                        let dialog = adw::AlertDialog::new(
                            Some("niri rejected the new config"),
                            Some(
                                "The compositor kept the previous config running. The new file is \
                                 on disk; run 'niri validate' in a terminal to see the error, or \
                                 Revert here and try again.",
                            ),
                        );
                        dialog.add_responses(&[("ok", "OK")]);
                        dialog.present(Some(&ui.win));
                    }
                    Confirmation::ValidatedDisk => {
                        toast(&ui, "Applied. The config on disk passes niri validate");
                    }
                    Confirmation::NoNiri => {
                        toast(
                            &ui,
                            &format!("Applied to {} (not validated: niri not found)", path.display()),
                        );
                    }
                    Confirmation::Unknown(msg) => {
                        toast(&ui, &format!("Applied, but no confirmation from niri: {msg}"));
                    }
                }
                refresh(&state, &ui);
                if let Some(f) = on_done {
                    f();
                }
            }
            Err(e) => {
                let dialog = adw::AlertDialog::new(Some("Could not save"), Some(&e.to_string()));
                dialog.add_responses(&[("ok", "OK")]);
                dialog.present(Some(&ui.win));
            }
        },
    );
}

pub fn toast(ui: &SharedUi, msg: &str) {
    ui.toasts.add_toast(adw::Toast::new(msg));
}

/// Handles for the option-style controls; wired with refresh() once the
/// shared Ui exists, so every change marks the model dirty in the UI (the
/// same treatment gesture rows get).
#[derive(Default)]
struct OptionControls {
    fingers: Option<adw::ComboRow>,
    horizontal: Option<adw::ComboRow>,
    touch_points: Option<adw::ComboRow>,
    debug_log: Option<adw::SwitchRow>,
    off: Option<adw::SwitchRow>,
}

fn wire_options(state: &SharedState, ui: &SharedUi, controls: &OptionControls) {
    if let Some(fingers) = &controls.fingers {
        fingers.connect_selected_notify({
            let state = state.clone();
            let ui = ui.clone();
            move |row| {
                if let Some(s) = row.selected_item().and_downcast::<gtk::StringObject>() {
                    if let Ok(n) = s.string().parse::<u8>() {
                        state.borrow_mut().model.options.fingers = Some(n);
                        refresh(&state, &ui);
                    }
                }
            }
        });
    }
    if let Some(hs) = &controls.horizontal {
        hs.connect_selected_notify({
            let state = state.clone();
            let ui = ui.clone();
            move |row| {
                state.borrow_mut().model.options.horizontal_swipe = if row.selected() == 0 {
                    HorizontalSwipe::MoveView
                } else {
                    HorizontalSwipe::ResizeColumn
                };
                refresh(&state, &ui);
            }
        });
    }
    if let Some(stp) = &controls.touch_points {
        stp.connect_selected_notify({
            let state = state.clone();
            let ui = ui.clone();
            move |row| {
                state.borrow_mut().model.options.show_touch_points = match row.selected() {
                    0 => ShowTouchPoints::Off,
                    1 => ShowTouchPoints::Gestures,
                    _ => ShowTouchPoints::All,
                };
                refresh(&state, &ui);
            }
        });
    }
    if let Some(dbg) = &controls.debug_log {
        dbg.connect_active_notify({
            let state = state.clone();
            let ui = ui.clone();
            move |row| {
                state.borrow_mut().model.options.debug_log = row.is_active();
                refresh(&state, &ui);
            }
        });
    }
    if let Some(off) = &controls.off {
        off.connect_active_notify({
            let state = state.clone();
            let ui = ui.clone();
            move |row| {
                state.borrow_mut().model.options.off = row.is_active();
                refresh(&state, &ui);
            }
        });
    }
}

/// Handles for the two multi-finger groups, whose titles and descriptions
/// are recomputed from the finger count on every refresh.
#[derive(Default)]
struct GroupHandles {
    base: Option<adw::PreferencesGroup>,
    more: Option<adw::PreferencesGroup>,
}

fn build_gesture_groups(content: &gtk::Box, state: &SharedState, rows: &mut HashMap<Slot, (adw::ActionRow, gtk::Label, gtk::Button)>, controls: &mut OptionControls, groups: &mut GroupHandles) {
    // Short row titles: the group carries the finger context.
    fn row_title(slot: Slot) -> &'static str {
        match slot {
            Slot::Tap => "Tap",
            Slot::TapMore => "Tap",
            Slot::SwipeMoreUp => "Flick up",
            Slot::SwipeMoreDown => "Flick down",
            Slot::SwipeMoreLeft => "Flick left",
            Slot::SwipeMoreRight => "Flick right",
            other => other.friendly_name(),
        }
    }

    // ---- The base finger-count group: everything driven by `fingers`.
    // Title/description are set by refresh() from the current count.
    let g3 = adw::PreferencesGroup::new();
    content.append(&g3);

    // Base finger count (defines what the two groups above are called).
    let fingers = adw::ComboRow::builder()
        .title("Finger count")
        .subtitle("Gestures start at this many fingers (3 or 4; default 3)")
        .build();
    let flist = gtk::StringList::new(&["3", "4"]);
    fingers.set_model(Some(&flist));
    // The parser resets out-of-range values to the default, but be defensive:
    // anything but 4 selects 3.
    let current = state.borrow().model.options.fingers.unwrap_or(3);
    fingers.set_selected(u32::from(current == 4));
    g3.add(&fingers);
    controls.fingers = Some(fingers);

    // Horizontal drag mode: a setting, but shown as the gesture it configures.
    let hs = adw::ComboRow::builder()
        .title("Horizontal swipe")
        .subtitle("The animated horizontal drag")
        .build();
    let hs_list = gtk::StringList::new(&["Move the view", "Resize the focused window"]);
    hs.set_model(Some(&hs_list));
    hs.set_selected(match state.borrow().model.options.horizontal_swipe {
        HorizontalSwipe::MoveView => 0,
        HorizontalSwipe::ResizeColumn => 1,
    });
    g3.add(&hs);
    controls.horizontal = Some(hs);

    // Vertical swipe: not remappable; say so instead of hiding it.
    let vrow = adw::ActionRow::builder()
        .title("Vertical swipe")
        .subtitle("Quick swipes run the built-in workspace carousel and can't be remapped; use the hold-swipes below for custom up or down actions")
        .build();
    let vlabel = gtk::Label::new(Some("Built-in"));
    vlabel.add_css_class("dim-label");
    vrow.add_suffix(&vlabel);
    g3.add(&vrow);

    for slot in [Slot::Tap, Slot::HoldLeft, Slot::HoldRight, Slot::HoldUp, Slot::HoldDown] {
        let (row, summary, test_btn) = gesture_row(slot, row_title(slot));
        g3.add(&row);
        rows.insert(slot, (row, summary, test_btn));
    }
    groups.base = Some(g3);

    // ---- The extra-finger tier: one finger more than the base count.
    // Title/description are set by refresh() from the current count.
    let g4 = adw::PreferencesGroup::new();
    content.append(&g4);
    for slot in [
        Slot::TapMore,
        Slot::SwipeMoreUp,
        Slot::SwipeMoreDown,
        Slot::SwipeMoreLeft,
        Slot::SwipeMoreRight,
    ] {
        let (row, summary, test_btn) = gesture_row(slot, row_title(slot));
        g4.add(&row);
        rows.insert(slot, (row, summary, test_btn));
    }
    groups.more = Some(g4);

    // ---- Edge swipes.
    let ge = adw::PreferencesGroup::new();
    ge.set_title("Edge swipes");
    ge.set_description(Some("One finger, swiping inward from a screen edge"));
    content.append(&ge);
    for slot in [Slot::EdgeBottom, Slot::EdgeTop, Slot::EdgeLeft, Slot::EdgeRight] {
        let (row, summary, test_btn) = gesture_row(slot, row_title(slot));
        ge.add(&row);
        rows.insert(slot, (row, summary, test_btn));
    }

    // ---- Corner swipes.
    let gc = adw::PreferencesGroup::new();
    gc.set_title("Corner swipes");
    gc.set_description(Some("One finger, swiping diagonally in from a corner"));
    content.append(&gc);
    for slot in [
        Slot::CornerTopLeft,
        Slot::CornerTopRight,
        Slot::CornerBottomLeft,
        Slot::CornerBottomRight,
    ] {
        let (row, summary, test_btn) = gesture_row(slot, row_title(slot));
        gc.add(&row);
        rows.insert(slot, (row, summary, test_btn));
    }
}

fn gesture_row(slot: Slot, title: &str) -> (adw::ActionRow, gtk::Label, gtk::Button) {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(slot.description())
        .activatable(true)
        .build();
    let summary = gtk::Label::new(None);
    summary.set_ellipsize(EllipsizeMode::End);
    summary.set_max_width_chars(24);
    summary.add_css_class("dim-label");
    row.add_suffix(&summary);
    let test_btn = gtk::Button::builder()
        .icon_name("system-run-symbolic")
        .css_classes(["flat"])
        .tooltip_text("Run this action now (via niri msg)")
        .build();
    row.add_suffix(&test_btn);
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    (row, summary, test_btn)
}

fn fire_test(ui: &SharedUi, state: &SharedState, slot: Slot) {
    let (niri_path, words) = {
        let s = state.borrow();
        let Some(niri_path) = s.niri.clone() else {
            return toast(ui, "niri not found on PATH");
        };
        if !s.ipc {
            return toast(ui, "no niri IPC socket available");
        }
        match catalog::msg_words(s.model.get(slot)) {
            Some(w) => (niri_path, w),
            None => return toast(ui, "this action can't be test-run"),
        }
    };
    let ui = ui.clone();
    super::blocking(
        move || niri::msg_action(&niri_path, &words),
        move |res| {
            if let Err(e) = res {
                toast(&ui, &format!("test failed: {e}"));
            }
        },
    );
}

fn build_options(content: &gtk::Box, state: &SharedState, controls: &mut OptionControls) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Options");

    let stp = adw::ComboRow::builder()
        .title("Show touch points")
        .subtitle("Dots that follow the fingers on screen")
        .build();
    let stp_list = gtk::StringList::new(&["Off", "While gesturing", "Always"]);
    stp.set_model(Some(&stp_list));
    stp.set_selected(match state.borrow().model.options.show_touch_points {
        ShowTouchPoints::Off => 0,
        ShowTouchPoints::Gestures => 1,
        ShowTouchPoints::All => 2,
    });
    group.add(&stp);
    controls.touch_points = Some(stp);

    let dbg = adw::SwitchRow::builder()
        .title("Gesture debug logging")
        .subtitle("Per-event gesture-debug: lines for bug reports")
        .active(state.borrow().model.options.debug_log)
        .build();
    group.add(&dbg);
    controls.debug_log = Some(dbg);

    let off = adw::SwitchRow::builder()
        .title("Disable multi-finger gestures")
        .subtitle("Turns off the drags, taps, flicks and holds")
        .active(state.borrow().model.options.off)
        .build();
    group.add(&off);
    controls.off = Some(off);

    content.append(&group);
}

fn refresh_banners(state: &SharedState, ui: &SharedUi) {
    while let Some(child) = ui.banner_box.first_child() {
        ui.banner_box.remove(&child);
    }
    let s = state.borrow();

    if s.niri.is_none() {
        ui.banner_box.append(&banner(
            "niri was not found on PATH; changes will be saved without validation",
            None,
            None,
        ));
    } else if !s.niri_supports_gestures {
        let state = state.clone();
        let b = banner(
            "This niri does not support the gestures config. Is the patched niri-tablet installed?",
            Some("Re-check"),
            Some(Box::new({
                let state = state.clone();
                let ui = ui.clone();
                move || recheck_niri(&state, &ui)
            })),
        );
        ui.banner_box.append(&b);
    }
    match &s.discovery.source {
        Source::None => {
            let state = state.clone();
            let b = banner(
                "No gestures config found. Applying here creates ~/.config/niri/cfg/gestures.kdl and adds the include line.",
                Some("Load shipped defaults"),
                Some(Box::new({
                    let state = state.clone();
                    let ui = ui.clone();
                    move || {
                        state.borrow_mut().model = crate::model::shipped_default();
                        refresh(&state, &ui);
                    }
                })),
            );
            ui.banner_box.append(&b);
        }
        Source::Inline(_) => {
            ui.banner_box.append(&banner(
                "Your gestures live inline in config.kdl. Applying moves them to ~/.config/niri/cfg/gestures.kdl (with a backup).",
                None,
                None,
            ));
        }
        Source::ManagedFileAmbiguous(_, _) => {
            ui.banner_box.append(&banner(
                "More than one file has a gestures block; managing the first (later includes override).",
                None,
                None,
            ));
        }
        _ => {}
    }
    for w in &s.load_warnings {
        ui.banner_box.append(&banner(w, None, None));
    }
    for w in &s.model.warnings {
        ui.banner_box.append(&banner(w, None, None));
    }
}

fn recheck_niri(state: &SharedState, ui: &SharedUi) {
    let state2 = state.clone();
    let ui = ui.clone();
    super::blocking(
        move || {
            let bin = niri::find_niri();
            let (ok, msg) = match &bin {
                Some(p) => niri::gestures_supported(p),
                None => (false, String::new()),
            };
            (bin, ok, msg)
        },
        move |(bin, ok, msg)| {
            {
                let mut s = state2.borrow_mut();
                s.niri = bin;
                s.niri_supports_gestures = ok;
            }
            if ok {
                toast(&ui, "niri supports the gestures config now");
            } else if !msg.is_empty() {
                toast(&ui, &format!("still unsupported: {msg}"));
            }
            refresh(&state2, &ui);
        },
    );
}

fn banner(text: &str, button: Option<&str>, on_button: Option<Box<dyn Fn() + 'static>>) -> adw::Banner {
    let b = adw::Banner::new(text);
    if let Some(label) = button {
        b.set_button_label(Some(label));
        if let Some(cb) = on_button {
            b.connect_button_clicked(move |_| cb());
        }
    }
    b.set_revealed(true);
    b
}
