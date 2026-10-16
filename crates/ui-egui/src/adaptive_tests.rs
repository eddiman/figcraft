use egui::{Rect, pos2, vec2};

use crate::CANVAS_MIN;
use crate::adaptive::{InputDensity, Layout, LayoutInput, LayoutMode, PanelPresentation, PanelSide, SafeInsets, layout};

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
