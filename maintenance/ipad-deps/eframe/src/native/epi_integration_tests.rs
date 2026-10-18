use super::*;
use egui::{Event, RawInput, ViewportCommand, ViewportEvent};

#[derive(Default)]
struct Probe {
    hooks: Vec<Vec<Event>>,
    delivered: Vec<Event>,
    logic_calls: usize,
    ui_calls: usize,
    close_requests: usize,
    cancel_close: bool,
    write_output: bool,
}

impl epi::App for Probe {
    fn raw_input_hook(&mut self, _: &egui::Context, raw: &mut RawInput) {
        for event in &mut raw.events {
            if let Event::Text(text) = event {
                text.push('!');
            }
        }
        self.hooks.push(raw.events.clone());
    }

    fn logic(&mut self, _: &egui::Context, _: &mut epi::Frame) {
        self.logic_calls += 1;
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut epi::Frame) {
        self.ui_calls += 1;
        self.delivered
            .extend(ui.input(|input| input.events.clone()));
        if ui.input(|input| input.viewport().close_requested()) {
            self.close_requests += 1;
            if self.cancel_close {
                ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
            }
        }
        if self.write_output {
            let text = self.ui_calls.to_string();
            ui.ctx().copy_text(text.clone());
            ui.ctx().send_viewport_cmd(ViewportCommand::Title(text));
        }
    }
}

fn integration() -> EpiIntegration {
    let egui_ctx = egui::Context::default();
    egui_ctx.options_mut(|options| {
        options.max_passes = core::num::NonZeroUsize::MIN;
    });
    EpiIntegration {
        frame: epi::Frame::_new_kittest(),
        last_auto_save: Instant::now(),
        beginning: Instant::now(),
        is_first_frame: true,
        egui_ctx,
        pending_raw_input: RawInput::default(),
        pending_full_output: egui::FullOutput::default(),
        close: false,
        can_drag_window: false,
        #[cfg(feature = "persistence")]
        persist_window: false,
        app_icon_setter: super::super::app_icon::AppTitleIconSetter::new(
            "Synthetic input test".into(),
            None,
        ),
    }
}

fn input(texts: impl IntoIterator<Item = String>) -> RawInput {
    RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        events: texts.into_iter().map(Event::Text).collect(),
        ..Default::default()
    }
}

#[test]
fn ipad_host_budget_retains_suffix_before_newer_input_without_replay() {
    let mut host = integration();
    let mut app = Probe::default();
    let count = RawInput::MAX_ORDERED_INPUT_BATCHES * 2 + 3;
    let output = host.update(&mut app, None, input((0..count).map(|n| n.to_string())));
    assert_eq!(app.hooks.len(), RawInput::MAX_ORDERED_INPUT_BATCHES);
    assert_eq!(host.pending_raw_input.events.len(), count - app.hooks.len());
    assert_eq!(
        output.viewport_output[&ViewportId::ROOT].repaint_delay,
        core::time::Duration::ZERO
    );
    output.drop_without_applying_deltas();

    host.update(&mut app, None, input(["new".into()]))
        .drop_without_applying_deltas();
    assert_eq!(app.hooks.len(), RawInput::MAX_ORDERED_INPUT_BATCHES * 2);
    host.update(&mut app, None, RawInput::default())
        .drop_without_applying_deltas();
    assert!(host.pending_raw_input.events.is_empty());
    let expected: Vec<_> = (0..count)
        .map(|n| Event::Text(format!("{n}!")))
        .chain([Event::Text("new!".into())])
        .collect();
    assert_eq!(app.delivered, expected);
    assert_eq!(app.hooks.len(), count + 1);
    assert_eq!(app.ui_calls, app.hooks.len());
    assert_eq!(app.logic_calls, app.hooks.len());
}

#[test]
fn ipad_host_hidden_ticks_do_not_normalize_or_replay_input_on_resume() {
    let mut host = integration();
    let mut app = Probe::default();
    for text in ["first", "second"] {
        let mut raw = input([text.into()]);
        raw.focused = false;
        let output = host.update_logic_only(&mut app, raw);
        assert!(output.viewport_commands.is_empty());
        assert!(!host.egui_ctx.input(|input| input.raw.focused));
    }
    assert_eq!(app.logic_calls, 2);
    assert_eq!(app.ui_calls, 0);
    assert!(app.hooks.is_empty());
    assert_eq!(host.pending_raw_input.events.len(), 2);

    host.update(&mut app, None, input(["third".into()]))
        .drop_without_applying_deltas();
    assert_eq!(
        app.delivered,
        ["first!", "second!", "third!"].map(|text| Event::Text(text.into()))
    );
    assert_eq!(app.hooks.len(), 3);
    assert_eq!(app.ui_calls, 3);
    assert_eq!(app.logic_calls, 5);
    assert!(host.pending_raw_input.events.is_empty());
}

#[test]
fn ipad_host_hidden_ticks_preserve_an_existing_redraw_suffix() {
    let mut host = integration();
    let mut app = Probe::default();
    let count = RawInput::MAX_ORDERED_INPUT_BATCHES + 3;
    host.update(&mut app, None, input((0..count).map(|n| n.to_string())))
        .drop_without_applying_deltas();
    assert_eq!(app.hooks.len(), RawInput::MAX_ORDERED_INPUT_BATCHES);
    for text in ["hidden-a", "hidden-b"] {
        let mut raw = input([text.into()]);
        raw.focused = false;
        assert!(
            host.update_logic_only(&mut app, raw)
                .viewport_commands
                .is_empty()
        );
    }
    assert_eq!(app.hooks.len(), RawInput::MAX_ORDERED_INPUT_BATCHES);
    assert_eq!(host.pending_raw_input.events.len(), 5);
    host.update(&mut app, None, input(["resumed".into()]))
        .drop_without_applying_deltas();
    let expected: Vec<_> = (0..count)
        .map(|n| Event::Text(format!("{n}!")))
        .chain(["hidden-a!", "hidden-b!", "resumed!"].map(|text| Event::Text(text.into())))
        .collect();
    assert_eq!(app.delivered, expected);
    assert_eq!(app.hooks.len(), count + 3);
    assert!(host.pending_raw_input.events.is_empty());
}

#[test]
fn ipad_host_merges_every_batch_output_in_dispatch_order() {
    let mut host = integration();
    let mut app = Probe {
        write_output: true,
        ..Default::default()
    };
    let output = host.update(&mut app, None, input(["a".into(), "b".into(), "c".into()]));
    let copied: Vec<_> = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, ["1", "2", "3"]);
    let titles: Vec<_> = output.viewport_output[&ViewportId::ROOT]
        .commands
        .iter()
        .filter_map(|command| match command {
            ViewportCommand::Title(title) => Some(title.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(titles, ["1", "2", "3"]);
    output.drop_without_applying_deltas();
}

#[test]
fn ipad_host_cancelled_close_is_one_shot_and_keeps_later_input() {
    let mut host = integration();
    let mut app = Probe {
        cancel_close: true,
        ..Default::default()
    };
    let mut raw = input(["a".into(), "b".into(), "c".into()]);
    raw.viewports
        .get_mut(&ViewportId::ROOT)
        .unwrap()
        .events
        .push(ViewportEvent::Close);
    host.update(&mut app, None, raw)
        .drop_without_applying_deltas();
    assert!(!host.should_close());
    assert_eq!(app.close_requests, 1);
    assert_eq!(app.delivered.len(), 3);
    assert!(host.pending_raw_input.events.is_empty());
}

#[test]
fn ipad_host_accepted_close_stops_before_later_batches() {
    let mut host = integration();
    let mut app = Probe::default();
    let mut raw = input(["a".into(), "b".into(), "c".into()]);
    raw.viewports
        .get_mut(&ViewportId::ROOT)
        .unwrap()
        .events
        .push(ViewportEvent::Close);
    host.update(&mut app, None, raw)
        .drop_without_applying_deltas();
    assert!(host.should_close());
    assert_eq!(app.close_requests, 1);
    assert_eq!(app.delivered, [Event::Text("a!".into())]);
    assert_eq!(app.hooks.len(), 1);
    assert!(host.pending_raw_input.events.is_empty());
}
