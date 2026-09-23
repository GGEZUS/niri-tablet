//! The action picker: searchable list of apps, curated niri actions, command
//! and custom-KDL editors, with live Test buttons.

use super::window::{self, SharedUi};
use super::SharedState;
use crate::catalog::{self, Args, Entry};
use crate::kdl;
use crate::model::{ActionNode, ActionValue, JsonArg, Slot};
use crate::{apps, niri};
use adw::prelude::*;
use std::rc::Rc;

struct Searchable {
    row: adw::ActionRow,
    haystack: String,
}

struct Group {
    group: adw::PreferencesGroup,
    items: Vec<Searchable>,
}

pub fn open(win: &adw::ApplicationWindow, state: &SharedState, ui: &SharedUi, slot: Slot) {
    let dialog = adw::Dialog::builder()
        .title(format!("{}: pick an action", slot.friendly_name()))
        .content_height(640)
        .content_width(460)
        .build();

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Search apps and actions…"));
    search.set_hexpand(true);

    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let header_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    header_box.set_margin_top(6);
    header_box.set_margin_start(6);
    header_box.set_margin_end(6);
    header_box.append(&header);
    header_box.append(&search);
    toolbar.add_top_bar(&header_box);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(6);
    content.set_margin_bottom(12);

    let mut groups: Vec<Group> = Vec::new();
    let toasts = adw::ToastOverlay::new();

    build_general_group(win, state, ui, slot, &dialog, &toasts, &mut groups);
    build_apps_group(state, ui, slot, &dialog, &toasts, &mut groups);
    build_catalog_groups(win, state, ui, slot, &dialog, &toasts, &mut groups);

    for g in &groups {
        content.append(&g.group);
    }

    let clamp = adw::Clamp::new();
    clamp.set_child(Some(&content));
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_child(Some(&clamp));
    scrolled.set_vexpand(true);
    toasts.set_child(Some(&scrolled));
    toolbar.set_content(Some(&toasts));

    // Search filtering.
    {
        let groups_ptr: Rc<std::cell::RefCell<Vec<Group>>> =
            Rc::new(std::cell::RefCell::new(groups));
        search.connect_search_changed(move |entry| {
            let q = entry.text().to_lowercase();
            for g in groups_ptr.borrow_mut().iter_mut() {
                let mut any_visible = false;
                for item in g.items.iter_mut() {
                    let visible = q.is_empty() || item.haystack.contains(&q);
                    item.row.set_visible(visible);
                    any_visible |= visible;
                }
                g.group.set_visible(any_visible);
            }
        });
    }

    dialog.set_child(Some(&toolbar));
    dialog.present(Some(win));
}

fn pick(state: &SharedState, ui: &SharedUi, slot: Slot, value: ActionValue, dialog: &adw::Dialog) {
    state.borrow_mut().model.set(slot, value);
    window::refresh(state, ui);
    dialog.close();
}

fn test_button(state: &SharedState, toasts: &adw::ToastOverlay, value: ActionValue, note: Option<&'static str>) -> gtk::Button {
    let btn = gtk::Button::builder()
        .icon_name("system-run-symbolic")
        .css_classes(["flat"])
        .tooltip_text("Run this action now")
        .build();
    if let Some(note) = note {
        btn.set_sensitive(false);
        btn.set_tooltip_text(Some(note));
        return btn;
    }
    let words = catalog::msg_words(&value).unwrap_or_default();
    let state = state.clone();
    let toasts = toasts.clone();
    btn.connect_clicked(move |_| {
        let Some(niri_path) = state.borrow().niri.clone() else {
            toasts.add_toast(adw::Toast::new("niri not found on PATH"));
            return;
        };
        let words = words.clone();
        let toasts = toasts.clone();
        super::blocking(
            move || niri::msg_action(&niri_path, &words),
            move |res| {
                if let Err(e) = res {
                    toasts.add_toast(adw::Toast::new(&format!("test failed: {e}")));
                }
            },
        );
    });
    btn
}

fn build_general_group(
    win: &adw::ApplicationWindow,
    state: &SharedState,
    ui: &SharedUi,
    slot: Slot,
    dialog: &adw::Dialog,
    toasts: &adw::ToastOverlay,
    groups: &mut Vec<Group>,
) {
    let group = adw::PreferencesGroup::new();
    group.set_title("General");
    let mut items = Vec::new();

    // Unbind
    let unbind = adw::ActionRow::builder()
        .title("Unbind")
        .subtitle("leave this gesture with no action")
        .build();
    unbind.add_prefix(&gtk::Image::from_icon_name("list-remove-symbolic"));
    {
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        unbind.connect_activated(move |_| {
            pick(&state, &ui, slot, ActionValue::None, &dialog);
        });
    }
    items.push(Searchable {
        row: unbind.clone(),
        haystack: "unbind none clear".into(),
    });
    group.add(&unbind);

    // Run a command (argv)
    let cmd = adw::ActionRow::builder()
        .title("Run a command")
        .subtitle("spawn a program with arguments")
        .build();
    cmd.add_prefix(&gtk::Image::from_icon_name("utilities-terminal-symbolic"));
    {
        let win = win.clone();
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        cmd.connect_activated(move |_| {
            dialog.close();
            command_editor(&win, &state, &ui, slot, false);
        });
    }
    items.push(Searchable {
        row: cmd.clone(),
        haystack: "run command spawn program argv terminal".into(),
    });
    group.add(&cmd);

    // Run a shell command
    let sh = adw::ActionRow::builder()
        .title("Run a shell command")
        .subtitle("spawn-sh: pipes one line through the shell (~ and || work)")
        .build();
    sh.add_prefix(&gtk::Image::from_icon_name("utilities-terminal-symbolic"));
    {
        let win = win.clone();
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        sh.connect_activated(move |_| {
            dialog.close();
            command_editor(&win, &state, &ui, slot, true);
        });
    }
    items.push(Searchable {
        row: sh.clone(),
        haystack: "run shell command spawn-sh sh bash script".into(),
    });
    group.add(&sh);

    // Custom KDL
    let custom = adw::ActionRow::builder()
        .title("Custom KDL")
        .subtitle("write the action node yourself (any niri action)")
        .build();
    custom.add_prefix(&gtk::Image::from_icon_name("text-x-generic-symbolic"));
    {
        let win = win.clone();
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        custom.connect_activated(move |_| {
            dialog.close();
            custom_kdl_editor(&win, &state, &ui, slot);
        });
    }
    items.push(Searchable {
        row: custom.clone(),
        haystack: "custom kdl advanced raw node".into(),
    });
    group.add(&custom);

    let _ = toasts;
    groups.push(Group { group, items });
}

fn build_apps_group(state: &SharedState, ui: &SharedUi, slot: Slot, dialog: &adw::Dialog, toasts: &adw::ToastOverlay, groups: &mut Vec<Group>) {
    let group = adw::PreferencesGroup::new();
    group.set_title("Installed apps");
    let mut items = Vec::new();

    for app in apps::scan() {
        let row = adw::ActionRow::builder()
            .title(&app.name)
            .subtitle(app.argv.join(" "))
            .activatable(true)
            .build();
        let img = gtk::Image::new();
        if let Some(icon) = &app.icon {
            set_icon(&img, icon);
        } else {
            img.set_icon_name(Some("application-x-executable-symbolic"));
        }
        row.add_prefix(&img);
        row.add_suffix(&test_button(state, toasts, ActionValue::spawn(&app.argv), None));
        {
            let state = state.clone();
            let ui = ui.clone();
            let dialog = dialog.clone();
            let argv = app.argv.clone();
            row.connect_activated(move |_| {
                pick(&state, &ui, slot, ActionValue::spawn(&argv), &dialog);
            });
        }
        items.push(Searchable {
            row: row.clone(),
            haystack: format!("{} {}", app.name, app.argv.join(" ")).to_lowercase(),
        });
        group.add(&row);
    }
    // Don't show an empty header on systems with no scannable apps.
    if !items.is_empty() {
        groups.push(Group { group, items });
    }
}

fn build_catalog_groups(
    _win: &adw::ApplicationWindow,
    state: &SharedState,
    ui: &SharedUi,
    slot: Slot,
    dialog: &adw::Dialog,
    toasts: &adw::ToastOverlay,
    groups: &mut Vec<Group>,
) {
    let mut by_cat: std::collections::BTreeMap<&str, Vec<&Entry>> = Default::default();
    for e in catalog::ENTRIES {
        by_cat.entry(e.category.label()).or_default().push(e);
    }
    for (cat, entries) in by_cat {
        let group = adw::PreferencesGroup::new();
        group.set_title(cat);
        let mut items = Vec::new();
        for entry in entries {
            let row = adw::ActionRow::builder()
                .title(entry.label)
                .subtitle(match entry.args {
                    Args::None => entry.name.to_string(),
                    _ => format!("{} …", entry.name),
                })
                .activatable(true)
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("input-gaming-symbolic"));

            let value = catalog::to_action(entry, None);
            let test = {
                let state = state.clone();
                let toasts = toasts.clone();
                // Test with the default arg so it actually runs.
                let arg = match entry.args {
                    Args::SizeChange => Some("+10%"),
                    Args::WorkspaceRef => Some("2"),
                    Args::Fixed(w) => Some(w[0]),
                    Args::None => None,
                };
                let v = catalog::to_action(entry, arg);
                test_button(&state, &toasts, v, entry.test_note)
            };
            row.add_suffix(&test);

            {
                let state = state.clone();
                let ui = ui.clone();
                let dialog = dialog.clone();
                row.connect_activated(move |_| {
                    if entry.args == Args::None {
                        pick(&state, &ui, slot, value.clone(), &dialog);
                    } else {
                        dialog.close();
                        arg_editor(&state, &ui, slot, entry);
                    }
                });
            }
            items.push(Searchable {
                row: row.clone(),
                haystack: format!("{} {}", entry.label, entry.name).to_lowercase(),
            });
            group.add(&row);
        }
        groups.push(Group { group, items });
    }
}

fn set_icon(img: &gtk::Image, name: &str) {
    let Some(display) = gtk::gdk::Display::default() else {
        img.set_icon_name(Some("application-x-executable-symbolic"));
        return;
    };
    let theme = gtk::IconTheme::for_display(&display);
    let paintable = theme.lookup_icon(
        name,
        &[],
        32,
        1,
        gtk::TextDirection::Ltr,
        gtk::IconLookupFlags::empty(),
    );
    img.set_paintable(Some(&paintable));
}

fn arg_editor(state: &SharedState, ui: &SharedUi, slot: Slot, entry: &'static Entry) {
    let dialog = adw::Dialog::builder()
        .title(format!("{}: {}", slot.friendly_name(), entry.label))
        .content_width(420)
        .build();

    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let entry_w = gtk::Entry::new();
    let preview = gtk::Label::new(None);
    preview.add_css_class("dim-label");
    preview.set_wrap(true);
    preview.set_selectable(true);
    preview.set_xalign(0.0);

    let (preset_words, placeholder, initial): (Vec<&str>, &str, String) = match entry.args {
        Args::SizeChange => (
            vec!["+10%", "-10%", "50%", "100%"],
            "size change, e.g. +10%",
            "+10%".into(),
        ),
        Args::WorkspaceRef => (
            vec!["2", "3", "4"],
            "workspace number or \"name\"",
            "2".into(),
        ),
        Args::Fixed(words) => (words.to_vec(), "choose one", words[0].into()),
        Args::None => (vec![], "", String::new()),
    };

    if !preset_words.is_empty() && entry.args != Args::Fixed(&[]) {
        let presets = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for w in preset_words {
            if w.is_empty() {
                continue;
            }
            let b = gtk::ToggleButton::with_label(w);
            b.set_css_classes(&["pill"]);
            {
                let entry_w = entry_w.clone();
                b.connect_toggled(move |btn| {
                    if btn.is_active() {
                        entry_w.set_text(w);
                    }
                });
            }
            presets.append(&b);
        }
        box_.append(&presets);
    }
    entry_w.set_placeholder_text(Some(placeholder));
    entry_w.set_text(&initial);

    // Live preview.
    let make_value = |text: &str| -> ActionValue {
        catalog::to_action(entry, Some(text))
    };
    let update_preview = {
        let entry_w = entry_w.clone();
        let preview = preview.clone();
        move || {
            let text = entry_w.text().to_string();
            let v = make_value(&text);
            let kdl_text = match &v {
                ActionValue::Node(n) => n.to_kdl().render(0),
                _ => String::new(),
            };
            preview.set_text(&kdl_text);
        }
    };
    update_preview();
    entry_w.connect_changed({
        let update = update_preview.clone();
        move |_| update()
    });
    box_.append(&entry_w);
    box_.append(&gtk::Label::new(Some("Will be written as:")));
    box_.append(&preview);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::builder().label("Cancel").css_classes(["flat"]).build();
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| { dialog.close(); }
    });
    let confirm = gtk::Button::builder()
        .label("Use this")
        .css_classes(["suggested-action"])
        .build();
    let toolbar_bottom = gtk::Box::new(gtk::Orientation::Vertical, 0);
    {
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        let entry_w = entry_w.clone();
        confirm.connect_clicked(move |_| {
            let text = entry_w.text().to_string();
            let v = make_value(&text);
            state.borrow_mut().model.set(slot, v);
            window::refresh(&state, &ui);
            dialog.close();
        });
    }
    actions.append(&cancel);
    actions.append(&confirm);
    actions.set_margin_bottom(12);
    actions.set_margin_start(12);
    actions.set_margin_end(12);
    toolbar_bottom.append(&actions);
    toolbar.add_bottom_bar(&toolbar_bottom);
    toolbar.set_content(Some(&box_));

    dialog.set_child(Some(&toolbar));
    if let Some(root) = ui.win.root() {
        if let Ok(w) = root.downcast::<adw::ApplicationWindow>() {
            dialog.present(Some(&w));
            return;
        }
    }
    dialog.present(Some(&ui.win));
}

fn command_editor(win: &adw::ApplicationWindow, state: &SharedState, ui: &SharedUi, slot: Slot, shell: bool) {
    let dialog = adw::Dialog::builder()
        .title(format!(
            "{}: run {}",
            slot.friendly_name(),
            if shell { "a shell command" } else { "a command" }
        ))
        .content_width(460)
        .build();

    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let program = gtk::Entry::new();
    program.set_placeholder_text(Some(if shell { "command line" } else { "program" }));
    let args = gtk::Entry::new();
    args.set_placeholder_text(Some("arguments (space separated)"));
    args.set_visible(!shell);
    let program_label = gtk::Label::new(Some(if shell {
        "Command (run through the shell; ~, ||, && work)"
    } else {
        "Program"
    }));
    program_label.set_xalign(0.0);

    let preview = gtk::Label::new(None);
    preview.add_css_class("dim-label");
    preview.set_selectable(true);
    preview.set_xalign(0.0);
    preview.set_wrap(true);

    let make_value = move |program: &str, args: &str| -> Option<ActionValue> {
        let p = program.trim();
        if p.is_empty() {
            return None;
        }
        Some(if shell {
            ActionValue::spawn_sh(p)
        } else {
            let mut argv = vec![p.to_string()];
            argv.extend(
                shlex::split(args)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|a| !a.is_empty()),
            );
            ActionValue::spawn(&argv)
        })
    };

    let update = {
        let program = program.clone();
        let args = args.clone();
        let preview = preview.clone();
        move || {
            let v = make_value(&program.text(), &args.text());
            let text = match &v {
                Some(ActionValue::Node(n)) => n.to_kdl().render(0),
                _ => "…".into(),
            };
            preview.set_text(&text);
        }
    };
    update();
    program.connect_changed({
        let update = update.clone();
        move |_| update()
    });
    args.connect_changed({
        let update = update.clone();
        move |_| update()
    });

    box_.append(&program_label);
    box_.append(&program);
    if !shell {
        let args_label = gtk::Label::new(Some("Arguments"));
        args_label.set_xalign(0.0);
        box_.append(&args_label);
        box_.append(&args);
    }
    box_.append(&gtk::Label::new(Some("Will be written as:")));
    box_.append(&preview);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::builder().label("Cancel").css_classes(["flat"]).build();
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| { dialog.close(); }
    });
    let confirm = gtk::Button::builder()
        .label("Use this")
        .css_classes(["suggested-action"])
        .build();
    {
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        let program = program.clone();
        let args = args.clone();
        confirm.connect_clicked(move |_| {
            if let Some(v) = make_value(&program.text(), &args.text()) {
                state.borrow_mut().model.set(slot, v);
                window::refresh(&state, &ui);
            }
            dialog.close();
        });
    }
    actions.append(&cancel);
    actions.append(&confirm);
    actions.set_margin_bottom(12);
    actions.set_margin_start(12);
    actions.set_margin_end(12);
    let bottom = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bottom.append(&actions);
    toolbar.add_bottom_bar(&bottom);
    toolbar.set_content(Some(&box_));

    dialog.set_child(Some(&toolbar));
    dialog.present(Some(win));
}

fn custom_kdl_editor(win: &adw::ApplicationWindow, state: &SharedState, ui: &SharedUi, slot: Slot) {
    let dialog = adw::Dialog::builder()
        .title(format!("{}: custom KDL", slot.friendly_name()))
        .content_width(460)
        .content_height(360)
        .build();

    let box_ = gtk::Box::new(gtk::Orientation::Vertical, 12);
    box_.set_margin_top(12);
    box_.set_margin_bottom(12);
    box_.set_margin_start(12);
    box_.set_margin_end(12);

    let label = gtk::Label::new(Some(
        "One niri action node, exactly as it would appear inside the gesture braces. Example:\nspawn \"foot\" \"-e\" \"htop\"",
    ));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("dim-label");
    box_.append(&label);

    let view = gtk::TextView::new();
    view.set_monospace(true);
    view.set_top_margin(6);
    view.set_left_margin(6);
    view.set_right_margin(6);
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_height_request(140);
    scrolled.set_child(Some(&view));

    let error = gtk::Label::new(None);
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.add_css_class("error");

    box_.append(&scrolled);
    box_.append(&error);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    actions.set_halign(gtk::Align::End);
    let cancel = gtk::Button::builder().label("Cancel").css_classes(["flat"]).build();
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| { dialog.close(); }
    });
    let confirm = gtk::Button::builder()
        .label("Use this")
        .css_classes(["suggested-action"])
        .build();
    {
        let state = state.clone();
        let ui = ui.clone();
        let dialog = dialog.clone();
        let buffer = view.buffer().clone();
        let error = error.clone();
        confirm.connect_clicked(move |_| {
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            // Wrap in the slot's parent so the existing parser accepts it,
            // then pull the action back out.
            let wrapper = format!("x {{\n{}\n}}\n", text);
            match kdl::parse_document(&wrapper) {
                Ok(doc) => {
                    let children = doc.first().map(|n| n.children.clone()).unwrap_or_default();
                    if children.len() == 1 && children[0].children.is_empty() {
                        let v = ActionValue::Node(Box::new(ActionNode {
                            name: children[0].name.clone(),
                            args: children[0].args.iter().map(Into::into).collect(),
                            props: children[0]
                                .props
                                .iter()
                                .map(|(k, v)| (k.clone(), JsonArg::from(v)))
                                .collect(),
                        }));
                        state.borrow_mut().model.set(slot, v);
                        window::refresh(&state, &ui);
                        dialog.close();
                    } else if children.len() > 1 {
                        // Keep everything verbatim; niri validate will judge.
                        state.borrow_mut().model.set(
                            slot,
                            ActionValue::Raw { kdl: wrapper.trim().to_string() },
                        );
                        window::refresh(&state, &ui);
                        dialog.close();
                    } else {
                        error.set_text("Write exactly one action node.");
                    }
                }
                Err(e) => error.set_text(&format!("{e}")),
            }
        });
    }
    actions.append(&cancel);
    actions.append(&confirm);
    actions.set_margin_bottom(12);
    actions.set_margin_start(12);
    actions.set_margin_end(12);
    let bottom = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bottom.append(&actions);
    toolbar.add_bottom_bar(&bottom);
    toolbar.set_content(Some(&box_));

    dialog.set_child(Some(&toolbar));
    dialog.present(Some(win));
}
