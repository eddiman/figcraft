//! Keyboard handling: the shortcuts of [`figcraft_engine::keymap`], tool keys, nudging, opacity
//! digits and text editing keys.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use egui::{Context, Event, Key, Modifiers};
use figcraft_engine::keymap::{self, Platform};
use serde_json::json;

use crate::FigApp;

/// Two opacity digits typed within this long make one exact percentage ("4" then "5": 45%).
pub const OPACITY_CHAIN_SECS: f64 = 0.6;

/// 0: not known yet (use the build's platform), 1: macOS, 2: Windows/Linux.
static PLATFORM: AtomicU8 = AtomicU8::new(0);

/// The platform whose keys apply: the build's, or in the browser the one egui detected.
pub fn platform() -> Platform {
    match PLATFORM.load(Ordering::Relaxed) {
        1 => Platform::Mac,
        2 => Platform::Other,
        _ => Platform::current(),
    }
}

fn detected_platform(os: egui::os::OperatingSystem) -> Platform {
    if os.is_mac() { Platform::Mac } else { Platform::Other }
}

fn note_platform(ctx: &Context) {
    PLATFORM.store(if detected_platform(ctx.os()) == Platform::Mac { 1 } else { 2 }, Ordering::Relaxed);
}

/// The keymap's view of egui's modifiers (Ctrl off macOS sets egui's `command` too).
pub fn mods(m: Modifiers) -> keymap::Mods {
    keymap::Mods { shift: m.shift, alt: m.alt, cmd: m.command || m.mac_cmd, ctrl: m.ctrl }
}

/// Symbols a US keyboard types without Shift.
const UNSHIFTED_SYMBOLS: [Key; 11] = [
    Key::Slash,
    Key::Backslash,
    Key::Minus,
    Key::Equals,
    Key::Period,
    Key::Comma,
    Key::Quote,
    Key::Semicolon,
    Key::OpenBracket,
    Key::CloseBracket,
    Key::Backtick,
];

/// The binding a key fires in `context`. Shortcuts follow the character typed, named as on a US
/// keyboard: a symbol another layout types with Shift (`/` is Shift+7 on a German keyboard, where
/// the key's position differs from the symbol) counts without the Shift. Characters egui has no key
/// for (`ü`, `ж`) already arrive as the key at that position.
pub fn lookup(context: keymap::Context, key: Key, physical: Option<Key>, m: Modifiers) -> Option<&'static keymap::Binding> {
    let mut held = mods(m);
    if held.shift && physical.is_some_and(|p| p != key) && UNSHIFTED_SYMBOLS.contains(&key) {
        held.shift = false;
    }
    keymap::lookup(platform(), context, key.name(), held)
}

/// The key a menu shows for a command.
pub fn shortcut(id: &str) -> Option<&'static str> {
    keymap::primary(id, platform())
}

/// A tooltip naming a command's key on this platform ("Align left  ⌥A"; just the label unbound).
pub fn tip(label: &'static str, id: &str) -> &'static str {
    cached_tip(label, shortcut(id))
}

/// A tooltip naming a key ("Resources  ⇧I").
pub fn key_tip(label: &'static str, key: &'static str) -> &'static str {
    cached_tip(label, Some(key))
}

/// Panels ask for their tooltips every frame: each (label, key, platform) is built once.
fn cached_tip(label: &'static str, key: Option<&'static str>) -> &'static str {
    type Entry = (Platform, &'static str, &'static str, &'static str);
    static CACHE: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
    let Some(key) = key else { return label };
    let p = platform();
    let Ok(mut cache) = CACHE.lock() else { return label };
    if let Some(e) = cache.iter().find(|e| e.0 == p && e.1 == label && e.2 == key) {
        return e.3;
    }
    // A bounded set: one entry per tooltip in the code.
    let text: &'static str = Box::leak(format!("{label}  {}", keymap::display(key, p)).into_boxed_str());
    cache.push((p, label, key, text));
    text
}

/// Run the shortcuts this frame's keys press. `typing`: a field had the keyboard when the frame
/// began.
pub fn handle(app: &mut FigApp, ctx: &Context, typing: bool) {
    note_platform(ctx);
    // Text fields in panels own the keyboard (also the key that ends their editing).
    if typing || crate::a11y::field_focused(ctx) || app.ui.palette.is_some() {
        return;
    }
    let events: Vec<Event> = ctx.input(|i| i.events.clone());
    if events.is_empty() {
        return;
    }
    if events.iter().any(|event| {
        matches!(event, Event::Text(_) | Event::Ime(_) | Event::Key { .. } | Event::Paste(_) | Event::OwnedPaste(_) | Event::Copy | Event::Cut)
    }) {
        app.cancel_canvas_pointer();
    }
    let editing = app.session.ds().map(|d| d.text_edit.is_some()).unwrap_or(false);
    if editing {
        text_keys(app, ctx, &events);
        return;
    }
    // An image-only clipboard brings no paste event, just the key: paste from the key then.
    let paste_event = events.iter().any(|e| matches!(e, Event::Paste(_) | Event::OwnedPaste(_)));
    let paste_key = events.iter().any(|e| matches!(e, Event::Key { key: Key::V, pressed: true, repeat: false, modifiers, .. } if modifiers.command && !modifiers.shift && !modifiers.alt));
    if paste_key && !paste_event {
        crate::menus::paste(app, None);
    }
    let now = ctx.input(|i| i.time);
    for ev in &events {
        canvas_event(app, ctx, ev, now);
    }
}

fn canvas_event(app: &mut FigApp, ctx: &Context, ev: &Event, now: f64) {
    match ev {
        Event::Copy => crate::menus::invoke(app, "edit.copy"),
        Event::Cut => crate::menus::invoke(app, "edit.cut"),
        Event::Paste(text) => crate::menus::paste(app, Some(text)),
        Event::OwnedPaste(outcome) => crate::menus::paste_owned(app, outcome),
        Event::Key { key, physical_key, pressed: true, modifiers, repeat } if key_event(app, *key, *physical_key, *modifiers, *repeat, now, ctx) => {
            // handled
        }
        _ => {}
    }
}

/// One raw batch's keyboard state for an explicitly enrolled ordered consumer.
/// Values do not retain any text or IME event across callbacks or frames.
pub(crate) struct KeyboardFrame {
    skip_text: bool,
    has_paste: bool,
    fallback_used: bool,
    edited_text: bool,
}
impl KeyboardFrame {
    pub(crate) fn new(events: &[Event]) -> Self {
        Self {
            skip_text: false,
            has_paste: events.iter().any(|e| matches!(e, Event::Paste(_) | Event::OwnedPaste(_))),
            fallback_used: false,
            edited_text: false,
        }
    }
    pub(crate) fn event(&mut self, app: &mut FigApp, ctx: &Context, event: &Event, now: f64) {
        app.cancel_canvas_pointer();
        if app.session.ds().is_ok_and(|d| d.text_edit.is_some()) {
            self.edited_text = true;
            text_key_events(app, std::slice::from_ref(event), &mut self.skip_text);
        } else {
            let paste_key = matches!(event, Event::Key { key: Key::V, pressed: true, repeat: false, modifiers, .. } if modifiers.command && !modifiers.shift && !modifiers.alt);
            if paste_key && !self.has_paste && !self.fallback_used {
                self.fallback_used = true;
                crate::menus::paste(app, None);
            }
            canvas_event(app, ctx, event, now);
        }
    }
    pub(crate) fn finish(self, app: &mut FigApp, ctx: &Context) {
        if self.edited_text {
            // raw_input_hook still sees the preceding pass's pointer clicks.
            // Only text presentation can run here; the ordinary frame handles
            // current mouse clicks through the unchanged legacy route.
            app.sync_preedit();
            ime_output(app, ctx);
        }
    }
}

/// Run the command a shortcut is bound to, with the keyboard's own twists (Escape steps out
/// before it deselects, Delete ends a pen path, Enter edits the selected text or vector).
pub fn run_binding(app: &mut FigApp, id: &str) {
    app.cancel_canvas_pointer();
    match id {
        "select.none" => {
            if let Err(e) = app.session.escape() {
                app.toast(e.to_string());
            }
            app.ui.context_menu = None;
        }
        "edit.delete" if app.session.tool.pen.is_some() => {
            let _ = app.session.pen_finish();
        }
        // Enter with a shape or frame tool chosen places one in the middle of the view.
        "select.children" if app.session.tool.tool.draws_shape() => {
            app.run("tool.place", json!({}));
        }
        "select.children" if enter_text(app) => {}
        _ => crate::menus::invoke(app, id),
    }
}

fn key_event(app: &mut FigApp, key: Key, physical: Option<Key>, m: Modifiers, repeat: bool, now: f64, ctx: &Context) -> bool {
    if key_box(app, key, m) {
        return true;
    }
    // A gradient open in the colour picker: Delete removes its selected stop.
    if matches!(key, Key::Backspace | Key::Delete) && m.is_none() && app.ui.picker.is_some() && crate::handles::remove_selected_stop(app, ctx) {
        return true;
    }
    // Vector edit mode and the shape builder have their own keys (tools, nudging points, delete,
    // join, Enter to finish); the rest fall through.
    match app.session.tool_key(key.name(), m.shift, m.alt, m.command) {
        Ok(true) => return true,
        Ok(false) => {}
        Err(e) => {
            app.toast(e.to_string());
            return true;
        }
    }
    if let Some(b) = lookup(keymap::Context::Canvas, key, physical, m) {
        // Copy/cut/paste arrive as clipboard events on most platforms.
        if matches!(b.id, "edit.copy" | "edit.cut" | "edit.paste") {
            return true;
        }
        if !repeat || matches!(b.id, "edit.undo" | "edit.redo" | "view.zoomIn" | "view.zoomOut") {
            run_binding(app, b.id);
        }
        return true;
    }
    // Arrows move through the layers panel while it has the keyboard.
    if app.ui.layers_focus && !m.command && !m.alt && crate::layers::key_nav(app, key, m.shift) {
        return true;
    }
    // Arrow nudges; with nothing selected the arrows pan the view.
    let (dx, dy) = match key {
        Key::ArrowLeft => (-1.0, 0.0),
        Key::ArrowRight => (1.0, 0.0),
        Key::ArrowUp => (0.0, -1.0),
        Key::ArrowDown => (0.0, 1.0),
        _ => (0.0, 0.0),
    };
    if (dx, dy) != (0.0, 0.0) && !m.alt {
        if app.session.selection().is_empty() {
            app.run("view.pan", json!({"dx": dx, "dy": dy, "big": m.shift}));
        } else if m.command {
            // Cmd/Ctrl+arrows resize: right/down grow, left/up shrink.
            app.run("transform.nudgeSize", json!({"dw": dx, "dh": dy, "big": m.shift}));
        } else {
            app.run("transform.nudge", json!({"dx": dx, "dy": dy, "big": m.shift}));
        }
        return true;
    }
    if m.is_none() && !repeat && opacity_digit(app, key, now) {
        return true;
    }
    // Enter while the pen is drawing finishes it.
    if key == Key::Enter && app.session.tool.pen.is_some() {
        let _ = app.session.pen_finish();
        return true;
    }
    false
}

/// Keys while a selection box drawn from the keyboard shows (`select.area`, Alt+Space or Ctrl+Space,
/// F8): the arrows move it, Cmd/Ctrl+arrows or Shift+arrows move its far corner, sizing it (Shift
/// with Cmd/Ctrl: five times as far), by 10 screen pixels; Enter selects what it covers like a
/// marquee (Shift toggles, Cmd/Ctrl takes nested layers), Escape drops it.
fn key_box(app: &mut FigApp, key: Key, m: Modifiers) -> bool {
    let Some(mut b) = app.ui.key_box else { return false };
    let size = m.shift || m.command;
    let step = if m.shift && m.command { 50.0 } else { 10.0 } / app.session.ds().map_or(1.0, |d| d.view.zoom.max(1e-6));
    let (dx, dy) = match key {
        Key::ArrowLeft => (-step, 0.0),
        Key::ArrowRight => (step, 0.0),
        Key::ArrowUp => (0.0, -step),
        Key::ArrowDown => (0.0, step),
        _ => (0.0, 0.0),
    };
    if (dx, dy) != (0.0, 0.0) {
        if !size {
            b[0] += dx;
            b[1] += dy;
        }
        b[2] += dx;
        b[3] += dy;
        app.ui.key_box = Some(b);
        return true;
    }
    match key {
        Key::Enter => {
            app.ui.key_box = None;
            let [x0, y0, x1, y1] = b;
            app.run("select.area", json!({"x": x0, "y": y0, "width": x1 - x0, "height": y1 - y0, "add": m.shift, "deep": m.command}));
            true
        }
        Key::Escape => {
            app.ui.key_box = None;
            true
        }
        _ => false,
    }
}

impl FigApp {
    /// Show a selection box for the keyboard: around the selection, else in the middle of the view.
    pub(crate) fn start_key_box(&mut self) {
        let Ok(ds) = self.session.ds() else { return };
        let zoom = ds.view.zoom.max(1e-6);
        let around = ds.selection.iter().filter_map(|i| ds.doc.world_bounds(*i)).reduce(|a, b| a.union(b));
        let r = match around {
            Some(r) => r.inflate(8.0 / zoom, 8.0 / zoom),
            None => {
                let c = self.to_page(self.canvas.rect.center());
                figcraft_geom::Rect::new(c.x - 100.0 / zoom, c.y - 75.0 / zoom, c.x + 100.0 / zoom, c.y + 75.0 / zoom)
            }
        };
        self.ui.key_box = Some([r.x0, r.y0, r.x1, r.y1]);
        let cmd = crate::toolbar::pretty_shortcut("Cmd");
        self.toast(crate::i18n::trf(
            "Selection box: arrows move it, Shift+arrows or {cmd}+arrows size it, Enter selects, Escape cancels",
            &[("cmd", &cmd)],
        ));
    }
}

/// The digit a number key types.
fn digit(key: Key) -> Option<u32> {
    let name = key.name();
    let mut chars = name.chars();
    let d = chars.next()?.to_digit(10)?;
    chars.next().is_none().then_some(d)
}

/// Number keys set the selection's opacity: one digit is tens of percent (0: 100%), a second digit
/// typed soon after makes the exact percentage.
pub fn opacity_digit(app: &mut FigApp, key: Key, now: f64) -> bool {
    let Some(d) = digit(key) else { return false };
    if !app.session.prefs.number_keys_opacity || app.session.selection().is_empty() {
        return false;
    }
    let first = app.ui.opacity_digit.take().filter(|(_, t)| now - t <= OPACITY_CHAIN_SECS).map(|(d, _)| d);
    let digits = match first {
        Some(f) => format!("{f}{d}"),
        None => {
            app.ui.opacity_digit = Some((d, now));
            d.to_string()
        }
    };
    app.run("object.opacityKeys", json!({"digits": digits}));
    true
}

/// Enter on a single selected text layer starts editing it; on shapes, edits them as vectors.
fn enter_text(app: &mut FigApp) -> bool {
    let sel = app.session.selection();
    if sel.len() > 1 {
        let ids = app.session.doc().ok().and_then(|d| figcraft_engine::vectoredit::enter_targets(d, &sel));
        return ids.is_some_and(|ids| {
            app.run("vector.edit.begin", json!({"ids": ids}));
            true
        });
    }
    if sel.len() != 1 {
        return false;
    }
    let Some(id) = sel.first().copied() else { return false };
    let kind = app.session.doc().ok().and_then(|d| d.node(id)).map(|n| (matches!(n.kind, figcraft_doc::NodeKind::Text(_)), n.kind.is_shape()));
    match kind {
        Some((true, _)) => {
            let _ = figcraft_engine::textedit::begin(&mut app.session, id, None, false);
            true
        }
        Some((_, true)) => {
            app.run("vector.edit.begin", json!({"id": id}));
            true
        }
        _ => false,
    }
}

fn text_keys(app: &mut FigApp, ctx: &Context, events: &[Event]) {
    let mut skip_text = false;
    text_key_events(app, events, &mut skip_text);
    finish_text_keys(app, ctx);
}

fn text_key_events(app: &mut FigApp, events: &[Event], skip_text: &mut bool) {
    for ev in events {
        match ev {
            Event::Text(t) | Event::Ime(egui::ImeEvent::Commit(t)) => {
                if *skip_text && matches!(ev, Event::Text(_)) {
                    *skip_text = false;
                    continue;
                }
                if let Err(e) = figcraft_engine::textedit::insert(&mut app.session, t) {
                    app.toast(e.to_string());
                }
            }
            // The composition shows inline at the caret; the document gets it on Commit.
            Event::Ime(egui::ImeEvent::Preedit { text, active_range_chars }) => {
                let cursor = active_range_chars.as_ref().map(|r| r.start);
                if let Err(e) = figcraft_engine::textedit::preedit(&mut app.session, text, cursor) {
                    app.toast(e.to_string());
                }
            }
            Event::OwnedPaste(outcome) => {
                if let egui::PasteOutcome::Content(content) = outcome {
                    if content.validate().is_ok()
                        && let Some(text) = &content.text
                        && let Err(error) = figcraft_engine::textedit::insert(&mut app.session, text)
                    {
                        app.toast(error.to_string());
                    }
                } else if let egui::PasteOutcome::Failed(error) = outcome {
                    app.toast(format!("Clipboard paste failed: {error:?}"));
                }
            }
            Event::Paste(t) => {
                // Plain text only: the engine normalises line endings and drops control chars.
                if let Err(e) = figcraft_engine::textedit::insert(&mut app.session, t) {
                    app.toast(e.to_string());
                }
            }
            Event::Copy | Event::Cut => {
                if let Some(text) = figcraft_engine::textedit::selected_text(&app.session).filter(|t| !t.is_empty()) {
                    let result = if app.services.clipboard_requires_confirmation() {
                        app.services.clipboard_try_set_text(&text)
                    } else {
                        app.services.clipboard_set_text(&text);
                        Ok(())
                    };
                    match result {
                        Ok(()) if matches!(ev, Event::Cut) => {
                            let _ = figcraft_engine::textedit::key(&mut app.session, "Delete", false, false, false);
                        }
                        Ok(()) => {}
                        Err(error) => app.toast(error),
                    }
                }
            }
            Event::Key { key, physical_key, pressed: true, modifiers: m, .. } => {
                if (m.command || m.alt || m.ctrl) && text_shortcut(app, *key, *physical_key, *m) {
                    *skip_text = true;
                    continue;
                }
                let name = match key {
                    Key::Backspace => "Backspace",
                    Key::Delete => "Delete",
                    Key::ArrowLeft => "Left",
                    Key::ArrowRight => "Right",
                    Key::ArrowUp => "Up",
                    Key::ArrowDown => "Down",
                    Key::Home => "Home",
                    Key::End => "End",
                    Key::Enter => "Enter",
                    Key::Escape => "Escape",
                    Key::Tab => "Tab",
                    Key::A if m.command => "SelectAll",
                    Key::Z if m.command => {
                        let _ = figcraft_engine::textedit::end(&mut app.session);
                        app.run(if m.shift { "edit.redo" } else { "edit.undo" }, json!({}));
                        continue;
                    }
                    _ => continue,
                };
                if name == "Enter" && m.command {
                    let _ = figcraft_engine::textedit::end(&mut app.session);
                    continue;
                }
                let (word, line) = keymap::text_motion(platform(), name, mods(*m));
                if let Err(e) = figcraft_engine::textedit::key(&mut app.session, name, m.shift, word, line) {
                    app.toast(e.to_string());
                }
            }
            _ => {}
        }
    }
}

fn finish_text_keys(app: &mut FigApp, ctx: &Context) {
    // Triple-click on the edited text selects its line (double-click is handled by the engine).
    if !app.canvas.native_values
        && ctx.input(|i| i.pointer.button_triple_clicked(egui::PointerButton::Primary))
        && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
    {
        triple_click_value(app, pos);
    }
    app.sync_preedit();
    ime_output(app, ctx);
}

/// The same line selection for a source-qualified physical triple click.
/// An enrolled pointer keeps this command within its checkpoint.
pub(crate) fn triple_click_value(app: &mut FigApp, pos: egui::Pos2) {
    if app.canvas.rect.contains(pos)
        && let Some(te) = app.session.ds().ok().and_then(|d| d.text_edit.clone())
    {
        let p = app.to_page(pos);
        let inside = app.session.doc().ok().and_then(|d| d.world_bounds(te.id)).is_some_and(|b| b.contains(p));
        if inside && let Ok(idx) = app.session.text_index_at(te.id, p) {
            app.pointer_command("text.edit.selectLine", json!({"index": idx}));
        }
    }
}

/// Shortcuts live while editing text (Bold, lists, alignment, size/weight/spacing steps, Save…).
fn text_shortcut(app: &mut FigApp, key: Key, physical: Option<Key>, m: Modifiers) -> bool {
    match lookup(keymap::Context::Text, key, physical, m) {
        Some(b) => {
            crate::menus::invoke(app, b.id);
            true
        }
        None => false,
    }
}

/// Tell the platform where the caret is (inside the composition while composing) so IME
/// candidate windows open next to it.
fn ime_output(app: &FigApp, ctx: &Context) {
    let Some(te) = app.session.ds().ok().and_then(|d| d.text_edit.clone()) else { return };
    let preedit = app.preedit_view();
    let Some(doc) = preedit.map(|p| &*p.doc).or_else(|| app.session.doc().ok()) else { return };
    let Some(node) = doc.node(te.id) else { return };
    let Some((layout, dy)) = figcraft_text::layout_node(node) else { return };
    let w = doc.world_transform(te.id);
    let (cx, li) = layout.caret(preedit.map(|p| p.cursor).unwrap_or(te.caret));
    let (top, h) = layout.lines.get(li).map(|l| (l.top, l.height)).unwrap_or((0.0, 14.0));
    let p0 = w * figcraft_geom::Point::new(cx, top + dy);
    let p1 = w * figcraft_geom::Point::new(cx, top + h + dy);
    let (a, b) = (app.to_screen(p0.x, p0.y), app.to_screen(p1.x, p1.y));
    if !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()) {
        return;
    }
    let cursor_rect = egui::Rect::from_two_pos(a, b + egui::vec2(1.0, 0.0));
    ctx.output_mut(|o| {
        o.ime = Some(egui::output::IMEOutput {
            purpose: egui::IMEPurpose::Normal,
            rect: app.canvas.rect,
            cursor_rect,
            should_interrupt_composition: false,
        })
    });
}

#[cfg(test)]
mod ipad_platform_tests {
    use super::*;

    #[test]
    fn ipad_apple_shortcut_mapping() {
        use egui::os::OperatingSystem as Os;
        for os in [Os::IOS, Os::Mac] {
            let platform = detected_platform(os);
            assert_eq!(platform, Platform::Mac);
            let command = mods(Modifiers::MAC_CMD | Modifiers::COMMAND);
            let control = mods(Modifiers::CTRL);
            for (key, id) in [("Z", "edit.undo"), ("C", "edit.copy"), ("V", "edit.paste"), ("S", "file.save")] {
                assert_eq!(keymap::lookup(platform, keymap::Context::Canvas, key, command).map(|binding| binding.id), Some(id));
                assert_ne!(keymap::lookup(platform, keymap::Context::Canvas, key, control).map(|binding| binding.id), Some(id));
            }
        }
        for os in [Os::Windows, Os::Nix, Os::Android, Os::Unknown] {
            let platform = detected_platform(os);
            assert_eq!(platform, Platform::Other);
            assert_eq!(
                keymap::lookup(platform, keymap::Context::Canvas, "S", mods(Modifiers::CTRL | Modifiers::COMMAND)).map(|binding| binding.id),
                Some("file.save")
            );
        }
        assert_eq!(Platform::current(), if cfg!(any(target_os = "macos", target_os = "ios")) { Platform::Mac } else { Platform::Other });
    }
}

#[cfg(test)]
mod ipad_clipboard_confirmed_tests {
    use super::*;
    use crate::Services;
    use figcraft_doc::NodeId;
    use figcraft_engine::{Session, textedit};
    use std::sync::{Arc, Mutex};

    struct Confirmed {
        result: Result<(), String>,
        writes: Arc<Mutex<Vec<String>>>,
    }
    impl Services for Confirmed {
        fn clipboard_requires_confirmation(&self) -> bool {
            true
        }
        fn clipboard_try_set_text(&mut self, text: &str) -> Result<(), String> {
            self.writes.lock().unwrap().push(text.to_owned());
            self.result.clone()
        }
        fn clipboard_set_text(&mut self, _: &str) {
            panic!("confirmed writer cannot use legacy output");
        }
    }
    fn app(result: Result<(), String>, editing: bool) -> (FigApp, NodeId, Arc<Mutex<Vec<String>>>) {
        let mut session = Session::new();
        let id = NodeId(session.execute("text.create", &json!({"text":"Åbc","x":0,"y":0})).unwrap()["id"].as_u64().unwrap());
        if editing {
            textedit::begin(&mut session, id, None, false).unwrap();
        }
        let writes = Arc::new(Mutex::new(Vec::new()));
        let app = FigApp::new(session, Box::new(Confirmed { result, writes: writes.clone() }));
        (app, id, writes)
    }
    #[test]
    fn ipad_clipboard_text_cut_denial_preserves_engine_state() {
        for error in ["denied", "unavailable", "too large"] {
            let (mut app, _, writes) = app(Err(error.into()), true);
            let before = app.session.ds().unwrap().clone();
            text_key_events(&mut app, &[Event::Cut], &mut false);
            let after = app.session.ds().unwrap();
            assert_eq!(after.doc, before.doc);
            assert_eq!(after.text_edit, before.text_edit);
            assert_eq!(after.selection, before.selection);
            assert_eq!(app.session.undo_depth(), (before.history.undo.len(), before.history.redo.len()));
            assert_eq!(*writes.lock().unwrap(), vec!["Åbc"]);
            assert_eq!(app.ui.toast.as_ref().map(|t| t.0.as_str()), Some(error));
        }
    }
    #[test]
    fn ipad_clipboard_text_cut_success_deletes_after_one_write() {
        let (mut app, id, writes) = app(Ok(()), true);
        text_key_events(&mut app, &[Event::Cut], &mut false);
        assert_eq!(app.session.doc().unwrap().node(id).unwrap().kind.text().unwrap().characters, "");
        assert_eq!(*writes.lock().unwrap(), vec!["Åbc"]);
    }
    #[test]
    fn ipad_clipboard_layer_failure_preserves_older_copy() {
        for command in ["edit.copy", "edit.cut"] {
            let (mut app, id, writes) = app(Err("denied".into()), false);
            app.session.execute("select.set", &json!({"ids":[id]})).unwrap();
            app.session.execute("edit.copy", &json!({})).unwrap();
            let other = app.session.execute("shape.rectangle", &json!({"x":10,"y":10,"width":20,"height":20})).unwrap();
            app.session.execute("select.set", &json!({"ids":[other["id"]]})).unwrap();
            app.ui.last_copy = Some("older marker".into());
            let before = app.session.ds().unwrap().clone();
            let old = app.session.clipboard.clone();
            crate::menus::invoke(&mut app, command);
            let after = app.session.ds().unwrap();
            assert_eq!(after.doc, before.doc);
            assert_eq!(after.selection, before.selection);
            assert_eq!(app.session.undo_depth(), (before.history.undo.len(), before.history.redo.len()));
            let clip = &app.session.clipboard;
            assert_eq!(clip.nodes, old.nodes);
            assert_eq!(clip.images, old.images);
            assert_eq!(clip.props, old.props);
            assert_eq!(clip.bounds, old.bounds);
            assert_eq!(clip.source_doc, old.source_doc);
            assert_eq!(clip.source, old.source);
            assert_eq!(clip.source_path, old.source_path);
            assert_eq!(app.ui.last_copy.as_deref(), Some("older marker"));
            assert_eq!(writes.lock().unwrap().len(), 1);
            assert_eq!(app.ui.toast.as_ref().map(|t| t.0.as_str()), Some("denied"));
        }
    }
    #[test]
    fn ipad_clipboard_layer_cut_success_records_matching_copy() {
        let (mut app, id, writes) = app(Ok(()), false);
        app.session.execute("select.set", &json!({"ids":[id]})).unwrap();
        let undo = app.session.undo_depth().0;
        crate::menus::invoke(&mut app, "edit.cut");
        assert!(app.session.doc().unwrap().node(id).is_none());
        assert_eq!(app.session.undo_depth().0, undo + 1);
        let writes = writes.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(app.ui.last_copy.as_ref(), writes.first());
        assert!(!app.session.clipboard.nodes.is_empty());
    }
    #[test]
    fn ipad_clipboard_confirmation_default_fails_closed() {
        struct Missing;
        impl Services for Missing {
            fn clipboard_requires_confirmation(&self) -> bool {
                true
            }
        }
        let (mut app, _, _) = app(Ok(()), true);
        app.services = Box::new(Missing);
        let before = app.session.doc().unwrap().clone();
        text_key_events(&mut app, &[Event::Cut], &mut false);
        assert_eq!(app.session.doc().unwrap(), &before);
        assert_eq!(app.ui.toast.as_ref().map(|t| t.0.as_str()), Some("Clipboard is unavailable"));
    }
}

#[cfg(test)]
mod ipad_owned_paste_tests {
    use super::*;
    use crate::Services;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct NoFallback;
    impl Services for NoFallback {
        fn clipboard_requires_confirmation(&self) -> bool {
            true
        }
        fn clipboard_get_text(&mut self) -> Option<String> {
            panic!("owned Paste cannot reread text");
        }
        fn clipboard_get_image(&mut self) -> Option<(u32, u32, Vec<u8>)> {
            panic!("owned Paste cannot reread image");
        }
    }
    #[test]
    fn ipad_clipboard_paste_once_and_failure_preserves_older_layers() {
        let mut app = FigApp::new(figcraft_engine::Session::new(), Box::new(NoFallback));
        app.run("shape.rectangle", json!({"x":0,"y":0,"width":10,"height":10}));
        app.run("edit.copy", json!({}));
        let before = app.session.ds().unwrap().clone();
        for outcome in [egui::PasteOutcome::Empty, egui::PasteOutcome::Unsupported, egui::PasteOutcome::Failed(egui::PasteError::Denied)] {
            crate::menus::paste_owned(&mut app, &outcome);
            assert_eq!(app.session.ds().unwrap().doc, before.doc);
            assert_eq!(app.session.ds().unwrap().selection, before.selection);
            assert_eq!(app.session.undo_depth(), (before.history.undo.len(), before.history.redo.len()));
        }
        let text = egui::PasteOutcome::Content(egui::PasteContent { text: Some("external text".into()), image: None, identity: None });
        let ctx = Context::default();
        let mut batch = KeyboardFrame::new(&[Event::OwnedPaste(text.clone()), Event::OwnedPaste(text.clone())]);
        batch.event(&mut app, &ctx, &Event::OwnedPaste(text.clone()), 0.0);
        batch.event(&mut app, &ctx, &Event::OwnedPaste(text), 0.0);
        let texts = app.session.doc().unwrap().nodes().filter(|n| n.kind.text().is_some_and(|t| t.characters == "external text")).count();
        assert_eq!(texts, 2);
    }
    #[test]
    fn ipad_menu_paste_requests_one_owned_outcome() {
        struct Menu(Arc<AtomicUsize>);
        impl Services for Menu {
            fn clipboard_requires_confirmation(&self) -> bool {
                true
            }
            fn clipboard_read_owned(&mut self) -> Option<egui::PasteOutcome> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Some(egui::PasteOutcome::Failed(egui::PasteError::Unavailable))
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let mut app = FigApp::new(figcraft_engine::Session::new(), Box::new(Menu(calls.clone())));
        let before = app.session.doc().unwrap().clone();
        crate::menus::invoke(&mut app, "edit.paste");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(app.session.doc().unwrap(), &before);
    }
}
