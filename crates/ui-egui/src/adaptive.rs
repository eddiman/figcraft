//! Pure iPad presentation policy. Callers keep panel preferences and native geometry.
//! Desktop panel fitting remains independent. T19 connects these choices to shared panels.
use egui::{Rect, Vec2, pos2, vec2};

use crate::{CANVAS_MIN, LEFT_PANEL, PANEL_DEFAULT, RIGHT_PANEL};

pub const MEDIUM_WIDTH: f32 = 760.0;
pub const WIDE_WIDTH: f32 = 1100.0;
const MAX_EXTENT: f32 = 1_000_000.0;
const MIN_DOCK_HEIGHT: f32 = 160.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Wide,
    Medium,
    Narrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputDensity {
    Pointer,
    Touch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SafeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Size and occlusion are logical points in window coordinates, before safe-area subtraction.
/// Keyboard geometry may be docked or floating. Invalid rectangles are ignored.
#[derive(Clone, Copy, Debug)]
pub struct LayoutInput {
    pub window_size: Vec2,
    pub safe_insets: SafeInsets,
    pub keyboard: Option<Rect>,
    pub density: InputDensity,
    pub panel_widths: (f32, f32),
    pub left_visible: bool,
    pub right_visible: bool,
    /// The preferred dock when only one side fits. The other visible side remains a drawer.
    pub medium_panel: PanelSide,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanelPresentation {
    Hidden,
    Drawer { button: Rect },
    Docked { bounds: Rect },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub mode: LayoutMode,
    /// Largest rectangular region outside the keyboard and safe insets.
    pub available: Rect,
    pub toolbar: Rect,
    pub canvas: Rect,
    pub left: PanelPresentation,
    pub right: PanelPresentation,
    /// Desired target size. Extremely small windows can provide less space.
    pub control_size: f32,
}

fn extent(value: f32) -> f32 {
    if value.is_finite() { value.clamp(0.0, MAX_EXTENT) } else { 0.0 }
}

fn panel_width(value: f32, limits: std::ops::RangeInclusive<f32>) -> f32 {
    if value.is_finite() { value.clamp(*limits.start(), *limits.end()) } else { PANEL_DEFAULT }
}

/// Compute geometry without reading or writing session/document/preferences state.
pub fn layout(input: &LayoutInput) -> Layout {
    let size = vec2(extent(input.window_size.x), extent(input.window_size.y));
    let insets = input.safe_insets;
    let min = pos2(extent(insets.left).min(size.x), extent(insets.top).min(size.y));
    let max = pos2((size.x - extent(insets.right)).max(min.x), (size.y - extent(insets.bottom)).max(min.y));
    let available = avoid_keyboard(Rect::from_min_max(min, max), input.keyboard);
    let mode = if available.width() >= WIDE_WIDTH {
        LayoutMode::Wide
    } else if available.width() >= MEDIUM_WIDTH {
        LayoutMode::Medium
    } else {
        LayoutMode::Narrow
    };
    let control_size: f32 = match input.density {
        InputDensity::Pointer => 28.0,
        InputDensity::Touch => 44.0,
    };
    let toolbar = Rect::from_min_size(available.min, vec2(available.width(), control_size.min(available.height())));
    let body = Rect::from_min_max(pos2(available.left(), toolbar.bottom()), available.max);
    let visible_count = u8::from(input.left_visible) + u8::from(input.right_visible);
    let button_width = if visible_count == 0 { 0.0 } else { control_size.min(toolbar.width() / f32::from(visible_count)) };
    let left_button = Rect::from_min_size(toolbar.min, vec2(button_width, toolbar.height()));
    let right_button = Rect::from_min_max(pos2(toolbar.right() - button_width, toolbar.top()), toolbar.max);
    let mut left = if input.left_visible { PanelPresentation::Drawer { button: left_button } } else { PanelPresentation::Hidden };
    let mut right = if input.right_visible { PanelPresentation::Drawer { button: right_button } } else { PanelPresentation::Hidden };
    let mut canvas = body;
    // If the keyboard leaves little height, keep all remaining body space for the canvas.
    if body.height() >= MIN_DOCK_HEIGHT && mode != LayoutMode::Narrow {
        let both = mode == LayoutMode::Wide;
        let dock_left = input.left_visible && (both || input.medium_panel == PanelSide::Left || !input.right_visible);
        let dock_right = input.right_visible && (both || input.medium_panel == PanelSide::Right || !input.left_visible);
        let left_min = if dock_left { *LEFT_PANEL.start() } else { 0.0 };
        let right_width =
            if dock_right { panel_width(input.panel_widths.1, RIGHT_PANEL).min((body.width() - left_min - CANVAS_MIN).max(0.0)) } else { 0.0 };
        let left_width =
            if dock_left { panel_width(input.panel_widths.0, LEFT_PANEL).min((body.width() - right_width - CANVAS_MIN).max(0.0)) } else { 0.0 };
        if dock_left {
            left = PanelPresentation::Docked { bounds: Rect::from_min_size(body.min, vec2(left_width, body.height())) };
        }
        if dock_right {
            right = PanelPresentation::Docked { bounds: Rect::from_min_max(pos2(body.right() - right_width, body.top()), body.max) };
        }
        canvas = Rect::from_min_max(pos2(body.left() + left_width, body.top()), pos2(body.right() - right_width, body.bottom()));
    }
    Layout { mode, available, toolbar, canvas, left, right, control_size }
}

fn avoid_keyboard(area: Rect, keyboard: Option<Rect>) -> Rect {
    let Some(keyboard) = keyboard.filter(|r| r.min.is_finite() && r.max.is_finite() && r.is_positive()) else { return area };
    let cut = area.intersect(keyboard);
    if !cut.is_positive() {
        return area;
    }
    // Deterministic ties prefer the region above the keyboard, then below, left, right.
    let candidates = [
        Rect::from_min_max(area.min, pos2(area.right(), cut.top())),
        Rect::from_min_max(pos2(area.left(), cut.bottom()), area.max),
        Rect::from_min_max(area.min, pos2(cut.left(), area.bottom())),
        Rect::from_min_max(pos2(cut.right(), area.top()), area.max),
    ];
    let mut best = Rect::from_min_size(area.min, Vec2::ZERO);
    for candidate in candidates {
        if candidate.area() > best.area() {
            best = candidate;
        }
    }
    best
}
