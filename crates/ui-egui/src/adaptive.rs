//! Pure iPad presentation policy. Callers keep panel preferences and native geometry.
//! Desktop panel fitting remains independent.
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

/// Presentation-only text scale. This is deliberately separate from Dynamic Type: Figcraft
/// keeps its document typography independent from the size of editor controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiTextScale {
    #[default]
    Percent100,
    Percent150,
    Percent200,
}

impl UiTextScale {
    pub const ALL: [Self; 3] = [Self::Percent100, Self::Percent150, Self::Percent200];

    pub const fn factor(self) -> f32 {
        match self {
            Self::Percent100 => 1.0,
            Self::Percent150 => 1.5,
            Self::Percent200 => 2.0,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Percent100 => "100%",
            Self::Percent150 => "150%",
            Self::Percent200 => "200%",
        }
    }
}

pub struct Presentation {
    pub enabled: bool,
    pub text_scale: UiTextScale,
    /// Window-point geometry supplied by the native host, never inferred from safe areas.
    pub keyboard: Option<Rect>,
    pub drawer: Option<PanelSide>,
    drawer_opened_this_frame: bool,
    pub multi_select: bool,
    pub constrain: bool,
    /// The dock which remains visible at a medium logical width.
    pub medium_panel: PanelSide,
    /// A user can hide either dock without changing its saved width or the desktop layout.
    pub left_hidden: bool,
    pub right_hidden: bool,
    pub last_layout: Option<Layout>,
    pub(crate) actions: Vec<(&'static str, Rect)>,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            enabled: false,
            text_scale: UiTextScale::default(),
            keyboard: None,
            drawer: None,
            drawer_opened_this_frame: false,
            multi_select: false,
            constrain: false,
            medium_panel: PanelSide::Right,
            left_hidden: false,
            right_hidden: false,
            last_layout: None,
            actions: Vec::new(),
        }
    }
}

impl Presentation {
    pub fn finger_modifiers(&self, mut physical: egui::Modifiers) -> egui::Modifiers {
        if self.enabled {
            physical.shift |= self.multi_select || self.constrain;
        }
        physical
    }
}

pub fn touch(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new("ipad_touch_density"))).unwrap_or(false)
}

fn scale(ctx: &egui::Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new("ipad_ui_scale"))).unwrap_or(1.0)
}

pub(crate) fn install_metrics(ctx: &egui::Context, presentation: &Presentation) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("ipad_touch_density"), presentation.enabled);
        d.insert_temp(egui::Id::new("ipad_ui_scale"), if presentation.enabled { presentation.text_scale.factor() } else { 1.0 });
    });
}

/// Scale an editor metric only while the iPad presentation is active.
pub fn metric(ui: &egui::Ui, value: f32) -> f32 {
    if touch(ui.ctx()) { value * scale(ui.ctx()) } else { value }
}

pub fn font_size(ui: &egui::Ui, value: f32) -> f32 {
    metric(ui, value)
}

pub fn target_size(ui: &egui::Ui, pointer: f32) -> f32 {
    if touch(ui.ctx()) { metric(ui, pointer.max(44.0)) } else { pointer }
}

pub(crate) fn button(app: &mut crate::FigApp, ui: &mut egui::Ui, key: &'static str, label: &str, selected: bool) -> bool {
    let control = target_size(ui, 44.0);
    let response = ui.add(egui::Button::new(label).selected(selected).min_size(vec2(control, control)));
    crate::a11y::button(&response, label);
    if response.has_focus() {
        response.scroll_to_me(None);
    }
    app.ui.adaptive.actions.push((key, response.rect.intersect(ui.clip_rect())));
    response.clicked()
}

pub(crate) fn drawer(app: &mut crate::FigApp, side: Option<PanelSide>) {
    if app.ui.adaptive.drawer != side {
        app.cancel_canvas_pointer();
        app.ui.adaptive.drawer = side;
        app.ui.adaptive.drawer_opened_this_frame = side.is_some();
    }
}

pub(crate) fn reveal_panel(app: &mut crate::FigApp, side: PanelSide) {
    match side {
        PanelSide::Left => {
            app.ui.left_collapsed = false;
            app.ui.adaptive.left_hidden = false;
        }
        PanelSide::Right => app.ui.adaptive.right_hidden = false,
    }
    if app.ui.adaptive.last_layout.is_some_and(|policy| {
        let presentation = if side == PanelSide::Left { policy.left } else { policy.right };
        matches!(presentation, PanelPresentation::Drawer { .. }) || (presentation == PanelPresentation::Hidden && policy.mode != LayoutMode::Wide)
    }) {
        drawer(app, Some(side));
    } else {
        app.ui.nav.go(if side == PanelSide::Left { crate::a11y::Region::Layers } else { crate::a11y::Region::Properties });
    }
}

fn toggle_panel(app: &mut crate::FigApp, side: PanelSide) {
    let docked = app.ui.adaptive.last_layout.is_some_and(|layout| match side {
        PanelSide::Left => matches!(layout.left, PanelPresentation::Docked { .. }),
        PanelSide::Right => matches!(layout.right, PanelPresentation::Docked { .. }),
    });
    let hidden = match side {
        PanelSide::Left => &mut app.ui.adaptive.left_hidden,
        PanelSide::Right => &mut app.ui.adaptive.right_hidden,
    };
    if docked && !*hidden {
        *hidden = true;
        drawer(app, None);
        app.cancel_canvas_pointer();
    } else {
        *hidden = false;
        reveal_panel(app, side);
    }
}

fn text_scale_menu(app: &mut crate::FigApp, ui: &mut egui::Ui) {
    use crate::i18n::tr;
    let control = target_size(ui, 44.0);
    let (response, menu) =
        egui::containers::menu::MenuButton::from_button(egui::Button::new(tr("Text")).min_size(vec2(control, control))).ui(ui, |ui| {
            for scale in UiTextScale::ALL {
                let selected = app.ui.adaptive.text_scale == scale;
                let response = ui.add_sized([control, control], egui::Button::new(scale.label()).selected(selected));
                crate::a11y::toggle(&response, scale.label(), selected);
                if response.clicked() {
                    app.ui.adaptive.text_scale = scale;
                    ui.close();
                }
            }
        });
    crate::a11y::menu_button(&response, tr("Text"), menu.is_some());
    app.ui.adaptive.actions.push(("text_scale", response.rect.intersect(ui.clip_rect())));
}

fn panel(app: &mut crate::FigApp, ui: &mut egui::Ui, side: PanelSide) {
    use crate::{a11y, i18n::tr};
    let (region, label) = match side {
        PanelSide::Left => (a11y::Region::Layers, tr("Layers")),
        PanelSide::Right => (a11y::Region::Properties, tr("Properties")),
    };
    a11y::region(&mut app.ui.nav, ui, region, egui::accesskit::Role::Pane, label);
    match side {
        PanelSide::Left => crate::layers::left_panel(app, ui),
        PanelSide::Right => crate::right::right_panel(app, ui),
    }
}

pub(crate) fn show(app: &mut crate::FigApp, ui: &mut egui::Ui, hidden: bool) {
    use crate::{i18n::tr, theme::Tokens};
    let ctx = ui.ctx().clone();
    install_metrics(&ctx, &app.ui.adaptive);
    let control = target_size(ui, 44.0);
    ui.spacing_mut().interact_size = vec2(control, control);
    app.ui.adaptive.actions.clear();
    let full = ui.available_rect_before_wrap().intersect(ctx.content_rect());
    // These reservations mirror the panels below. The pure fit policy therefore sees the space
    // which is actually left for docks and the canvas, including a two-row tool shelf.
    let header_height = control * if hidden { 1.0 } else { 2.0 };
    let keyboard = app.ui.adaptive.keyboard.map(|r| r.translate(-full.min.to_vec2()));
    let usable_height = avoid_keyboard(Rect::from_min_size(egui::Pos2::ZERO, full.size()), keyboard).height();
    let compact_tools = usable_height < header_height + control * 5.0;
    let tool_height = if hidden {
        0.0
    } else if compact_tools {
        // A horizontal scroll row also reserves its scroll affordance. Keeping that space in the
        // reservation prevents a 44-point child from being clipped when the keyboard is present.
        control + metric(ui, 24.0)
    } else {
        control * 2.0 + 12.0
    };
    let policy = layout(&LayoutInput {
        window_size: full.size(),
        safe_insets: SafeInsets::default(),
        keyboard,
        density: InputDensity::Touch,
        panel_widths: (app.ui.left_width.max(metric(ui, 360.0)), app.ui.right_width.max(metric(ui, 360.0))),
        left_visible: !hidden && !app.ui.left_collapsed && !app.ui.adaptive.left_hidden,
        right_visible: !hidden && !app.ui.adaptive.right_hidden,
        medium_panel: app.ui.adaptive.medium_panel,
        header_height,
        tool_height,
    });
    let available = policy.available.translate(full.min.to_vec2());
    if app.ui.adaptive.last_layout.is_some_and(|old| old != policy) {
        app.cancel_canvas_pointer();
    }
    app.ui.adaptive.last_layout = Some(policy);
    if let Some(side) = app.ui.adaptive.drawer {
        let presentation = match side {
            PanelSide::Left => policy.left,
            PanelSide::Right => policy.right,
        };
        if !matches!(presentation, PanelPresentation::Drawer { .. }) {
            drawer(app, None);
        }
    }
    let t = Tokens::get(&ctx);
    ui.scope_builder(egui::UiBuilder::new().max_rect(available), |ui| {
        crate::tabs::tab_bar(app, ui);
        if !hidden {
            egui::Panel::top("ipad_primary").exact_size(control).frame(egui::Frame::NONE.fill(t.panel)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (key, label, command) in
                        [("save", tr("Save"), "file.save"), ("undo", tr("Undo"), "edit.undo"), ("redo", tr("Redo"), "edit.redo")]
                    {
                        if button(app, ui, key, label, false) {
                            crate::menus::invoke(app, command);
                        }
                    }
                    // Estimate the complete row before choosing its disclosure. This keeps the
                    // save history controls visible while every panel remains reachable.
                    let expanded = ui.available_width() >= control * 5.0 + 220.0;
                    if expanded {
                        if button(app, ui, "layers", tr("Layers"), !app.ui.adaptive.left_hidden && app.ui.left_tab == crate::LeftTab::File) {
                            app.ui.left_tab = crate::LeftTab::File;
                            toggle_panel(app, PanelSide::Left);
                        }
                        if button(app, ui, "assets", tr("Assets"), !app.ui.adaptive.left_hidden && app.ui.left_tab == crate::LeftTab::Assets) {
                            app.ui.left_tab = crate::LeftTab::Assets;
                            toggle_panel(app, PanelSide::Left);
                        }
                        if button(app, ui, "properties", tr("Properties"), !app.ui.adaptive.right_hidden) {
                            toggle_panel(app, PanelSide::Right);
                        }
                        if button(app, ui, "actions", tr("Actions"), false) {
                            app.cancel_canvas_pointer();
                            app.ui.palette = Some(String::new());
                        }
                    } else {
                        let (response, menu) = egui::containers::menu::MenuButton::from_button(
                            egui::Button::new(tr("Panels")).min_size(vec2(control, control)),
                        )
                        .ui(ui, |ui| {
                            if button(app, ui, "layers", tr("Layers"), false) {
                                app.ui.left_tab = crate::LeftTab::File;
                                reveal_panel(app, PanelSide::Left);
                                ui.close();
                            }
                            if button(app, ui, "assets", tr("Assets"), false) {
                                app.ui.left_tab = crate::LeftTab::Assets;
                                reveal_panel(app, PanelSide::Left);
                                ui.close();
                            }
                            if button(app, ui, "properties", tr("Properties"), false) {
                                reveal_panel(app, PanelSide::Right);
                                ui.close();
                            }
                        });
                        crate::a11y::menu_button(&response, tr("Panels"), menu.is_some());
                        app.ui.adaptive.actions.push(("panels", response.rect.intersect(ui.clip_rect())));
                        let (response, menu) = egui::containers::menu::MenuButton::from_button(
                            egui::Button::new(tr("More")).min_size(vec2(control, control)),
                        )
                        .ui(ui, |ui| {
                            if button(app, ui, "actions", tr("Actions"), false) {
                                app.cancel_canvas_pointer();
                                app.ui.palette = Some(String::new());
                                ui.close();
                            }
                        });
                        crate::a11y::menu_button(&response, tr("More"), menu.is_some());
                        app.ui.adaptive.actions.push(("primary_more", response.rect.intersect(ui.clip_rect())));
                    }
                    text_scale_menu(app, ui);
                });
            });
            egui::Panel::bottom("ipad_tools").exact_size(tool_height).frame(egui::Frame::NONE.fill(t.panel)).show(ui, |ui| {
                crate::a11y::region(&mut app.ui.nav, ui, crate::a11y::Region::Toolbar, egui::accesskit::Role::Toolbar, tr("Tools"));
                crate::toolbar::touch_show(app, ui, compact_tools);
            });
        }
        for (side, presentation) in [(PanelSide::Left, policy.left), (PanelSide::Right, policy.right)] {
            if let PanelPresentation::Docked { bounds } = presentation {
                let dock = match side {
                    PanelSide::Left => egui::Panel::left("ipad_left"),
                    PanelSide::Right => egui::Panel::right("ipad_right"),
                };
                dock.exact_size(bounds.width()).resizable(false).frame(egui::Frame::NONE.fill(t.panel)).show(ui, |ui| panel(app, ui, side));
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.canvas)).show(ui, |ui| {
            if app.ui.adaptive.drawer.is_some() {
                ui.disable();
            }
            let mut worker = app.worker.take().unwrap_or_else(|| crate::render_worker::Worker::new(ctx.clone()));
            crate::canvas::show(app, ui, &mut worker);
            app.worker = Some(worker);
        });
    });
    if let Some(side) = app.ui.adaptive.drawer {
        let width = match side {
            PanelSide::Left => app.ui.left_width,
            PanelSide::Right => app.ui.right_width,
        };
        let width = width.max(360.0).min((available.width() - 24.0).max(0.0));
        let position = pos2(if side == PanelSide::Left { available.left() } else { available.right() - width }, available.top());
        let id = egui::Id::new(("ipad_drawer", side == PanelSide::Left));
        let response = egui::Modal::new(id)
            .area(
                egui::Modal::default_area(id)
                    .anchor(egui::Align2::LEFT_TOP, position.to_vec2() - ctx.content_rect().min.to_vec2())
                    .default_size(vec2(width, available.height()))
                    .constrain_to(available),
            )
            .frame(egui::Frame::NONE.fill(t.panel))
            .show(&ctx, |ui| {
                ui.set_width(width);
                ui.set_height(available.height());
                if button(app, ui, "close_drawer", tr("Close panel"), false) {
                    drawer(app, None);
                }
                egui::ScrollArea::vertical().id_salt("drawer_scroll").max_height((available.height() - 52.0).max(0.0)).show(ui, |ui| {
                    panel(app, ui, side);
                });
            });
        // Opening and releasing a header control happens in one egui frame. Ignore that release
        // once, then retain the normal Escape and outside-tap dismissal on later frames.
        let opened_this_frame = std::mem::take(&mut app.ui.adaptive.drawer_opened_this_frame);
        if !opened_this_frame && response.should_close() {
            drawer(app, None);
        }
    }
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
    /// The tab and primary-action rows reserved above the editor body.
    pub header_height: f32,
    /// The touch tool rows reserved below the editor body.
    pub tool_height: f32,
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
    let header_height = extent(input.header_height).min(available.height());
    let toolbar = Rect::from_min_size(available.min, vec2(available.width(), header_height));
    let tool_height = extent(input.tool_height).min((available.height() - toolbar.height()).max(0.0));
    let body = Rect::from_min_max(pos2(available.left(), toolbar.bottom()), pos2(available.right(), available.bottom() - tool_height));
    let visible_count = u8::from(input.left_visible) + u8::from(input.right_visible);
    let button_width = if visible_count == 0 { 0.0 } else { control_size.min(toolbar.width() / f32::from(visible_count)) };
    let button_height = control_size.min(toolbar.height());
    let left_button = Rect::from_min_size(toolbar.min, vec2(button_width, button_height));
    let right_button = Rect::from_min_size(pos2(toolbar.right() - button_width, toolbar.top()), vec2(button_width, button_height));
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
