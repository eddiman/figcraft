use egui::{Rect, pos2, vec2};

use crate::CANVAS_MIN;
use crate::adaptive::{InputDensity, Layout, LayoutInput, LayoutMode, PanelPresentation, PanelSide, SafeInsets, WIDE_WIDTH, layout};

fn input(width: f32) -> LayoutInput {
    LayoutInput {
        window_size: vec2(width, 800.0),
        safe_insets: SafeInsets::default(),
        keyboard: None,
        density: InputDensity::Touch,
        panel_widths: (240.0, 240.0),
        left_visible: true,
        right_visible: true,
        medium_panel: PanelSide::Right,
    }
}

fn check_geometry(l: Layout) {
    assert!(l.available.is_finite());
    assert!(l.available.contains_rect(l.toolbar));
    assert!(l.available.contains_rect(l.canvas));
    assert!(l.canvas.width() >= 0.0 && l.canvas.height() >= 0.0);
    assert!(!l.canvas.intersect(l.toolbar).is_positive());
    let mut panel_bounds = Vec::new();
    let mut buttons = Vec::new();
    for p in [l.left, l.right] {
        match p {
            PanelPresentation::Docked { bounds } => {
                assert!(l.available.contains_rect(bounds));
                assert!(!bounds.intersect(l.canvas).is_positive());
                assert!(!bounds.intersect(l.toolbar).is_positive());
                panel_bounds.push(bounds);
            }
            PanelPresentation::Drawer { button } => {
                assert!(l.toolbar.contains_rect(button));
                buttons.push(button);
            }
            PanelPresentation::Hidden => {}
        }
    }
    for rects in [panel_bounds, buttons] {
        if let [a, b] = rects.as_slice() {
            assert!(!a.intersect(*b).is_positive());
        }
    }
}

#[test]
fn ipad_layout_wide_medium_narrow() {
    for (width, mode, docked) in [(600.0, LayoutMode::Narrow, 0), (800.0, LayoutMode::Medium, 1), (1200.0, LayoutMode::Wide, 2)] {
        let l = layout(&input(width));
        assert_eq!(l.mode, mode);
        assert_eq!([l.left, l.right].iter().filter(|p| matches!(p, PanelPresentation::Docked { .. })).count(), docked);
        assert!(l.canvas.width() >= CANVAS_MIN);
        for p in [l.left, l.right] {
            if let PanelPresentation::Drawer { button } = p {
                assert_eq!(button.size(), vec2(44.0, 44.0));
            }
        }
        check_geometry(l);
    }
    for (width, mode) in [(759.99, LayoutMode::Narrow), (760.0, LayoutMode::Medium), (1099.99, LayoutMode::Medium), (1100.0, LayoutMode::Wide)] {
        assert_eq!(layout(&input(width)).mode, mode);
    }
    let l = layout(&LayoutInput { medium_panel: PanelSide::Left, ..input(800.0) });
    assert!(matches!(l.left, PanelPresentation::Docked { .. }));
    assert!(matches!(l.right, PanelPresentation::Drawer { .. }));
}

#[test]
fn ipad_layout_safe_area() {
    let mut i = input(1200.0);
    i.safe_insets = SafeInsets { top: 24.0, left: 60.0, right: 60.0, bottom: 20.0 };
    let l = layout(&i);
    assert_eq!(l.available, Rect::from_min_max(pos2(60.0, 24.0), pos2(1140.0, 780.0)));
    assert_eq!(l.mode, LayoutMode::Medium, "thresholds use available width");
    check_geometry(l);
    i.safe_insets.left = 5000.0;
    i.safe_insets.top = 5000.0;
    let l = layout(&i);
    assert_eq!(l.available.size(), vec2(0.0, 0.0));
    check_geometry(l);
}

#[test]
fn ipad_layout_keyboard_occlusion() {
    for keyboard in [
        Rect::from_min_max(pos2(0.0, 500.0), pos2(1200.0, 800.0)),
        Rect::from_min_max(pos2(350.0, 300.0), pos2(850.0, 600.0)),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 800.0)),
        Rect::from_min_max(pos2(0.0, 100.0), pos2(1200.0, 800.0)),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1200.0, 800.0)),
    ] {
        let l = layout(&LayoutInput { keyboard: Some(keyboard), ..input(1200.0) });
        assert!(!l.available.intersect(keyboard).is_positive());
        check_geometry(l);
        if l.canvas.height() < 160.0 {
            assert!(!matches!(l.left, PanelPresentation::Docked { .. }));
            assert!(!matches!(l.right, PanelPresentation::Docked { .. }));
        }
    }
    let outside = Rect::from_min_max(pos2(1300.0, 0.0), pos2(1500.0, 800.0));
    assert_eq!(layout(&LayoutInput { keyboard: Some(outside), ..input(1200.0) }), layout(&input(1200.0)));
}

#[test]
fn ipad_layout_does_not_overwrite_preferences() {
    let mut session = figcraft_engine::Session::new();
    session.prefs.panel_widths = (480.0, 480.0);
    let prefs_before = serde_json::to_value(&session.prefs).unwrap();
    for width in [600.0, 800.0, 1100.0, 1200.0] {
        let i = LayoutInput { panel_widths: session.prefs.panel_widths, ..input(width) };
        let l = layout(&i);
        assert!(l.canvas.width() >= CANVAS_MIN);
        assert_eq!(i.panel_widths, (480.0, 480.0));
        check_geometry(l);
    }
    assert_eq!(serde_json::to_value(&session.prefs).unwrap(), prefs_before);
    let restored = layout(&LayoutInput { panel_widths: session.prefs.panel_widths, ..input(1400.0) });
    assert!(matches!(restored.left, PanelPresentation::Docked { bounds } if bounds.width() == 480.0));
    assert!(matches!(restored.right, PanelPresentation::Docked { bounds } if bounds.width() == 480.0));
}

#[test]
fn ipad_layout_invalid_dimensions() {
    for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -100.0, 0.0, 1.0, 40.0, f32::MAX] {
        for y in [f32::NAN, -10.0, 0.0, 10.0, 44.0, 200.0, f32::MAX] {
            let mut i = input(x);
            i.window_size.y = y;
            i.panel_widths = (f32::NAN, f32::INFINITY);
            i.safe_insets = SafeInsets { left: f32::NAN, right: -10.0, top: f32::INFINITY, bottom: -20.0 };
            let l = layout(&i);
            check_geometry(l);
            let invalid = Rect::from_min_max(pos2(f32::NAN, 0.0), pos2(100.0, 100.0));
            assert_eq!(layout(&LayoutInput { keyboard: Some(invalid), ..i }), l);
        }
    }
}

#[test]
fn ipad_layout_density_and_hidden_panels() {
    for width in [40.0, 600.0, 800.0, 1200.0] {
        for (left_visible, right_visible) in [(false, false), (false, true), (true, false), (true, true)] {
            let i = LayoutInput { left_visible, right_visible, density: InputDensity::Pointer, ..input(width) };
            let l = layout(&i);
            assert_eq!(l.control_size, 28.0);
            assert_eq!(l.left == PanelPresentation::Hidden, !left_visible);
            assert_eq!(l.right == PanelPresentation::Hidden, !right_visible);
            check_geometry(l);
        }
    }
}
struct Editor {
    app: crate::FigApp,
    ctx: egui::Context,
    native: crate::touch_native::Native,
    size: egui::Vec2,
    time: f64,
    textures: std::collections::BTreeMap<egui::TextureId, egui::ColorImage>,
    pointer: Option<egui::Pos2>,
}

impl Editor {
    fn new(size: egui::Vec2) -> Self {
        let mut app = crate::FigApp::new(figcraft_engine::Session::new(), Box::new(crate::NoServices));
        app.ui.adaptive.enabled = true;
        app.ui.home = false;
        let mut editor =
            Self { app, ctx: egui::Context::default(), native: Default::default(), size, time: 0.0, textures: Default::default(), pointer: None };
        for _ in 0..4 {
            editor.frame(vec![]);
        }
        editor
    }
    fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.time += 0.05;
        let mut raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let frame = self.native.process(&mut self.app, &self.ctx, &mut raw);
        assert!(frame.issues.is_empty(), "{:?}", frame.issues);
        let mut output = self.ctx.run_ui(raw, |ui| self.app.ui(ui));
        self.native.finish_frame(&self.app, &self.ctx, true);
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                if let Some([x, y]) = delta.pos {
                    let texture = self.textures.get_mut(id).unwrap();
                    for row in 0..image.height() {
                        for col in 0..image.width() {
                            texture[(x + col, y + row)] = image[(col, row)];
                        }
                    }
                } else {
                    self.textures.insert(*id, image.as_ref().clone());
                }
            }
        }
        for id in &output.textures_delta.free {
            self.textures.remove(id);
        }
        output.textures_delta.clear();
        output
    }
    fn action(&self, key: &str) -> Rect {
        self.app.ui.adaptive.actions.iter().find(|(name, _)| *name == key).unwrap_or_else(|| panic!("missing action {key}")).1
    }
    fn click(&mut self, key: &str) {
        let p = self.action(key).center();
        if self.pointer.is_some() {
            self.pointer = Some(p);
        }
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE },
            ]);
        }
        self.frame(vec![]);
    }
    fn finger(&mut self, id: u64, phase: egui::TouchPhase, pos: egui::Pos2) {
        let mut events = vec![egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None }];
        match phase {
            egui::TouchPhase::Start => {
                self.pointer = Some(pos);
                events.push(egui::Event::PointerMoved(pos));
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            egui::TouchPhase::Move => {
                self.pointer = Some(pos);
                events.push(egui::Event::PointerMoved(pos));
            }
            egui::TouchPhase::End => {
                events.push(egui::Event::PointerButton {
                    pos: self.pointer.take().unwrap(),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
                events.push(egui::Event::PointerGone);
            }
            egui::TouchPhase::Cancel => events.push(egui::Event::PointerGone),
        }
        self.frame(events);
    }
}

#[test]
fn ipad_drawer_preserves_canvas_state() {
    let mut e = Editor::new(vec2(600.0, 900.0));
    e.app.run("shape.rectangle", serde_json::json!({"x": 0, "y": 0, "width": 50, "height": 40}));
    e.app.run("tool.set", serde_json::json!({"tool": "ellipse"}));
    e.frame(vec![]);
    let selection = e.app.session.selection();
    let prefs = e.app.session.prefs.clone();
    let view = e.app.session.ds().unwrap().view.clone();
    let doc = e.app.session.ds().unwrap().doc.clone();
    for side in [PanelSide::Left, PanelSide::Right] {
        e.click(if side == PanelSide::Left { "layers" } else { "properties" });
        assert_eq!(e.app.ui.adaptive.drawer, Some(side));
        e.click("close_drawer");
        assert!(e.app.ui.adaptive.drawer.is_none());
    }
    for size in [vec2(800.0, 600.0), vec2(1200.0, 800.0), vec2(600.0, 900.0)] {
        e.size = size;
        e.frame(vec![]);
    }
    let ds = e.app.session.ds().unwrap();
    assert!(std::sync::Arc::ptr_eq(&doc, &ds.doc));
    assert_eq!(e.app.session.selection(), selection);
    assert_eq!(e.app.session.tool.tool, figcraft_engine::ToolId::Ellipse);
    assert_eq!((ds.view.cx, ds.view.cy, ds.view.zoom), (view.cx, view.cy, view.zoom));
    assert_eq!(e.app.session.prefs, prefs);
}

#[test]
fn ipad_modal_blocks_canvas() {
    let mut e = Editor::new(vec2(600.0, 900.0));
    e.app.run("tool.set", serde_json::json!({"tool": "rectangle"}));
    let doc = e.app.session.ds().unwrap().doc.clone();
    let p = e.app.canvas.rect.center();
    e.finger(1, egui::TouchPhase::Start, p);
    e.finger(1, egui::TouchPhase::Move, p + vec2(50.0, 30.0));
    assert!(e.app.canvas_pointer.is_some());
    crate::adaptive::drawer(&mut e.app, Some(PanelSide::Right));
    e.frame(vec![]);
    assert!(crate::ordered_input::Target::capture(&e.app, &e.ctx, 1).is_none());
    e.finger(1, egui::TouchPhase::End, p + vec2(80.0, 60.0));
    assert!(std::sync::Arc::ptr_eq(&doc, &e.app.session.ds().unwrap().doc));
    assert!(e.app.canvas_pointer.is_none());
    let outside = pos2(30.0, 400.0);
    e.finger(2, egui::TouchPhase::Start, outside);
    e.finger(2, egui::TouchPhase::End, outside);
    e.frame(vec![]);
    assert!(std::sync::Arc::ptr_eq(&doc, &e.app.session.ds().unwrap().doc));
}

#[test]
fn ipad_touch_modifier_routes_existing_command() {
    let mut e = Editor::new(vec2(800.0, 900.0));
    e.app.run("tool.set", serde_json::json!({"tool": "rectangle"}));
    e.click("constrain");
    assert!(e.app.ui.adaptive.constrain);
    let p = e.app.canvas.rect.center() - vec2(60.0, 60.0);
    e.finger(1, egui::TouchPhase::Start, p);
    e.finger(1, egui::TouchPhase::Move, p + vec2(80.0, 30.0));
    e.finger(1, egui::TouchPhase::End, p + vec2(80.0, 30.0));
    let selected = e.app.session.selection();
    assert_eq!(selected.len(), 1);
    let node = e.app.session.doc().unwrap().node(selected[0]).unwrap();
    assert_eq!(node.width, node.height, "existing Shift-constrained rectangle");
    let undo = e.app.session.ds().unwrap().history.undo.len();
    e.click("duplicate");
    assert_eq!(e.app.session.ds().unwrap().history.undo.len(), undo + 1);
    e.click("undo");
    assert_eq!(e.app.session.ds().unwrap().history.undo.len(), undo);
    e.click("redo");
    assert_eq!(e.app.session.ds().unwrap().history.undo.len(), undo + 1);
    e.click("context");
    assert!(e.app.ui.context_menu.is_some());
}

#[test]
fn ipad_primary_actions_reachable() {
    for size in [vec2(600.0, 900.0), vec2(800.0, 600.0), vec2(1200.0, 800.0), vec2(600.0, 400.0)] {
        for keyboard in [None, Some(Rect::from_min_max(pos2(0.0, size.y - 240.0), size.to_pos2()))] {
            let mut e = Editor::new(size);
            e.app.ui.adaptive.keyboard = keyboard;
            e.frame(vec![]);
            let keys = [
                "save",
                "undo",
                "redo",
                "layers",
                "assets",
                "properties",
                "actions",
                "multi_select",
                "constrain",
                "duplicate",
                "context",
                "close_tab",
            ];
            let rects: Vec<_> = keys.into_iter().map(|key| (key, e.action(key))).collect();
            for (key, rect) in &rects {
                assert!(rect.width() >= 44.0 && rect.height() >= 44.0, "{size:?} {key}: {rect:?}");
                assert!(Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(*rect), "{key}");
                assert!(keyboard.is_none_or(|k| !k.intersect(*rect).is_positive()), "{key}");
            }
            for (i, (key, rect)) in rects.iter().enumerate() {
                for (other, r) in rects.iter().skip(i + 1) {
                    assert!(!rect.intersect(*r).is_positive(), "{key} overlaps {other}");
                }
            }
            assert!(e.app.canvas.rect.width() >= crate::CANVAS_MIN);
            assert!(e.app.canvas.rect.height() > 0.0);
        }
    }
}

#[test]
fn ipad_touch_numeric_binding_has_separate_allocated_targets() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("touch_numeric_geometry");
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("ipad_touch_density"), true));
        ui.horizontal(|ui| {
            crate::widgets::number_field_bindable(ui, id, crate::widgets::Lead::Text("X"), Some(100.0), "", 160.0, 1.0, Some(None), "");
            let icon = crate::icons::button(ui, crate::icons::Icon::Plus, 20.0, false, "");
            assert_eq!(icon.rect.size(), vec2(44.0, 44.0));
        });
    });
    output.textures_delta.clear();
    let field = ctx.read_response(id.with("field")).unwrap().rect;
    let binding = ctx.read_response(id.with("bind")).unwrap().rect;
    assert!(field.width() >= 44.0 && field.height() >= 44.0);
    assert_eq!(binding.size(), vec2(44.0, 44.0));
    assert!(!field.intersect(binding).is_positive());
}

#[test]
fn ipad_shared_panel_commands_reveal_drawers() {
    for width in [600.0, 800.0, 1200.0] {
        let mut e = Editor::new(vec2(width, 900.0));
        e.click("assets");
        assert_eq!(e.app.ui.left_tab, crate::LeftTab::Assets);
        assert_eq!(e.app.ui.adaptive.drawer, (width < WIDE_WIDTH).then_some(PanelSide::Left));
        if width < WIDE_WIDTH {
            e.click("close_drawer");
        }
        e.app.ui.left_collapsed = true;
        e.frame(vec![]);
        e.click("layers");
        assert_eq!(e.app.ui.left_tab, crate::LeftTab::File);
        assert!(!e.app.ui.left_collapsed);
        assert!(!e.app.ui.versions.open);
        assert_eq!(e.app.ui.adaptive.drawer, (width < WIDE_WIDTH).then_some(PanelSide::Left));
    }
}

#[test]
fn ipad_touch_latches_preserve_physical_modifiers_and_cancel_held_intent() {
    let mut e = Editor::new(vec2(800.0, 900.0));
    let physical = egui::Modifiers { command: true, alt: true, ctrl: true, mac_cmd: true, shift: true };
    assert_eq!(e.app.ui.adaptive.finger_modifiers(physical), physical);
    e.click("multi_select");
    assert!(e.app.ui.adaptive.finger_modifiers(egui::Modifiers::ALT).alt);
    assert!(e.app.ui.adaptive.finger_modifiers(egui::Modifiers::ALT).shift);
    e.app.run("tool.set", serde_json::json!({"tool": "rectangle"}));
    let p = e.app.canvas.rect.center() - vec2(40.0, 40.0);
    let doc = e.app.session.ds().unwrap().doc.clone();
    e.finger(1, egui::TouchPhase::Start, p);
    e.finger(1, egui::TouchPhase::Move, p + vec2(40.0, 20.0));
    assert!(e.app.canvas_pointer.is_some());
    e.click("constrain");
    e.finger(1, egui::TouchPhase::End, p + vec2(70.0, 50.0));
    assert!(std::sync::Arc::ptr_eq(&doc, &e.app.session.ds().unwrap().doc));
    assert!(e.app.canvas_pointer.is_none());
}

#[test]
fn ipad_multi_select_uses_finger_shift_without_changing_physical_mouse() {
    let mut e = Editor::new(vec2(800.0, 900.0));
    for x in [20.0, 160.0] {
        e.app.run("shape.rectangle", serde_json::json!({"x": x, "y": 0, "width": 60, "height": 60}));
    }
    e.app.run("select.none", serde_json::json!({}));
    e.frame(vec![]);
    e.click("multi_select");
    for (id, x) in [(1, 40.0), (2, 180.0)] {
        let p = e.app.to_screen(x, 20.0);
        e.finger(id, egui::TouchPhase::Start, p);
        e.finger(id, egui::TouchPhase::End, p);
    }
    assert_eq!(e.app.session.selection().len(), 2);
    let p = e.app.to_screen(280.0, 120.0);
    for pressed in [true, false] {
        e.frame(vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE },
        ]);
    }
    assert!(e.app.session.selection().is_empty(), "latch does not make a physical blank click additive");
}
