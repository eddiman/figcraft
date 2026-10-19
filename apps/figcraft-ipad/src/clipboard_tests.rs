//! Production dependency regressions for the iPad's opt-in focus and clipboard features.
use egui::{Context, Event, Id, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, TouchDeviceId, TouchId, TouchPhase, pos2, vec2};
use std::sync::{Arc, Mutex};

struct Fields {
    ctx: Context,
    text: [String; 2],
    rects: [Rect; 2],
    time: f64,
    reverse: bool,
}

impl Fields {
    fn new(reverse: bool) -> Self {
        let ctx = Context::default();
        ctx.set_os(egui::os::OperatingSystem::IOS);
        let mut fields = Self { ctx, text: ["a".into(), "b".into()], rects: [Rect::NOTHING; 2], time: 0.0, reverse };
        for _ in 0..3 {
            fields.frame(vec![]);
        }
        fields
    }

    fn id(index: usize) -> Id {
        Id::new(("ipad-field", index))
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        self.input_frame(RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        });
    }

    fn ordered_frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let mut pending = RawInput { time: Some(self.time), events, ..Default::default() };
        for _ in 0..RawInput::MAX_ORDERED_INPUT_BATCHES {
            self.input_frame(pending.take_ordered_input());
            if pending.events.is_empty() {
                return;
            }
        }
        panic!("test input exceeded one redraw budget");
    }

    fn input_frame(&mut self, input: RawInput) {
        let order = if self.reverse { [1, 0] } else { [0, 1] };
        self.ctx
            .run_ui(input, |ui| {
                for index in order {
                    self.rects[index] = ui
                        .put(
                            Rect::from_min_size(pos2(20.0, 20.0 + index as f32 * 100.0), vec2(280.0, 32.0)),
                            egui::TextEdit::singleline(&mut self.text[index]).id(Self::id(index)),
                        )
                        .rect;
                }
            })
            .drop_without_applying_deltas();
    }

    fn focus(&mut self, index: usize) {
        self.ctx.memory_mut(|memory| memory.request_focus(Self::id(index)));
        self.frame(vec![]);
    }

    fn tap(&self, index: usize) -> Vec<Event> {
        let pos = self.rects[index].center();
        vec![
            Event::Touch { device_id: TouchDeviceId(1), id: TouchId(1), phase: TouchPhase::Start, pos, force: None },
            Event::PointerMoved(pos),
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
            Event::Touch { device_id: TouchDeviceId(1), id: TouchId(1), phase: TouchPhase::End, pos, force: None },
            Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE },
            Event::PointerGone,
        ]
    }
}

fn paste(text: &str) -> Event {
    Event::OwnedPaste(egui::PasteOutcome::Content(egui::PasteContent { text: Some(text.into()), image: None, identity: None }))
}

#[test]
fn ipad_field_focus_owns_text_and_owned_paste_in_both_draw_orders() {
    for reverse in [false, true] {
        let mut fields = Fields::new(reverse);
        fields.focus(0);
        let mut events = vec![Event::Text("p".into())];
        events.extend(fields.tap(1));
        events.extend([paste("q"), paste("r")]);
        fields.frame(events);
        assert_eq!(fields.text, ["ap", "bqr"]);
    }
}

#[test]
fn ipad_native_batches_keep_interleaved_paste_with_its_field() {
    for reverse in [false, true] {
        let mut fields = Fields::new(reverse);
        fields.focus(0);
        let mut events = fields.tap(1);
        events.push(paste("q"));
        events.extend(fields.tap(0));
        events.push(paste("r"));
        fields.ordered_frame(events);
        assert_eq!(fields.text, ["ar", "bq"], "reverse={reverse}");
    }
}

#[test]
fn ipad_native_batches_preserve_escape_and_unfocused_prefix() {
    for reverse in [false, true] {
        let mut fields = Fields::new(reverse);
        let mut events = vec![paste("ignored")];
        events.extend(fields.tap(1));
        events.extend([
            Event::Text("q".into()),
            Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE },
            paste("ignored"),
        ]);
        fields.ordered_frame(events);
        assert_eq!(fields.text, ["a", "bq"]);
        assert!(fields.ctx.memory(|memory| memory.focused().is_none()));
    }
}

#[test]
fn ipad_native_batches_pass_through_single_native_panel_owner() {
    for reverse in [false, true] {
        let mut fields = Fields::new(reverse);
        fields.focus(0);
        let mut app = figcraft_ui_egui::FigApp::new(figcraft_engine::Session::new(), Box::new(figcraft_ui_egui::NoServices));
        let mut native = figcraft_ui_egui::touch_native::Native::default();
        let mut events = vec![Event::Text("p".into())];
        events.extend(fields.tap(1));
        events.push(paste("q"));
        events.extend(fields.tap(0));
        events.push(paste("r"));
        let mut pending = RawInput { time: Some(1.0), events, ..Default::default() };
        while !pending.events.is_empty() {
            let mut batch = pending.take_ordered_input();
            let frame = native.process(&mut app, &fields.ctx, &mut batch);
            assert!(frame.issues.is_empty(), "{:?}", frame.issues);
            fields.input_frame(batch);
            native.finish_frame(&app, &fields.ctx, true);
        }
        assert_eq!(fields.text, ["apr", "bq"]);
    }
}

#[test]
fn ipad_native_batches_preserve_cross_field_writer_order() {
    for reverse in [false, true] {
        let mut fields = Fields::new(reverse);
        fields.text = ["alpha".into(), "beta".into()];
        fields.focus(0);
        let select_all = || Event::Key {
            key: Key::A,
            physical_key: Some(Key::A),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::MAC_CMD | Modifiers::COMMAND,
        };
        fields.frame(vec![select_all()]);
        let writes = Arc::new(Mutex::new(Vec::new()));
        let observed = writes.clone();
        fields.ctx.set_text_clipboard_writer(Some(Arc::new(move |text| {
            observed.lock().unwrap().push(text);
            Ok(())
        })));
        let mut events = vec![Event::Copy];
        events.extend(fields.tap(1));
        events.extend([select_all(), Event::Cut]);
        events.extend(fields.tap(0));
        events.extend([select_all(), Event::Copy]);
        fields.ordered_frame(events);
        assert_eq!(fields.text, ["alpha", ""]);
        assert_eq!(*writes.lock().unwrap(), ["alpha", "beta", "alpha"]);
    }
}

#[test]
fn ipad_native_batches_match_sequential_repeated_taps_and_cursor_changes() {
    for reverse in [false, true] {
        let mut batched = Fields::new(reverse);
        let mut sequential = Fields::new(reverse);
        let operations = [batched.tap(1), vec![paste("first")], batched.tap(1), vec![Event::Text("second".into())]];
        let combined = operations.iter().flatten().cloned().collect();
        for operation in operations {
            sequential.frame(operation);
        }
        batched.ordered_frame(combined);
        assert_eq!(batched.text, sequential.text);
        assert_eq!(
            egui::TextEdit::load_state(&batched.ctx, Fields::id(1)).unwrap().cursor.char_range(),
            egui::TextEdit::load_state(&sequential.ctx, Fields::id(1)).unwrap().cursor.char_range()
        );
    }
}

#[test]
fn ipad_native_batch_budget_retains_suffix_and_appends_newer_events() {
    let mut fields = Fields::new(false);
    fields.focus(0);
    let mut pending = RawInput {
        time: Some(42.0),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
        events: (0..20).map(|n| paste(&format!("{n},"))).collect(),
        ..Default::default()
    };
    for index in 0..RawInput::MAX_ORDERED_INPUT_BATCHES {
        let batch = pending.take_ordered_input();
        assert_eq!(batch.time, Some(42.0));
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.screen_rect.is_some(), index == 0);
        fields.input_frame(batch);
    }
    assert_eq!(pending.events.len(), 12);
    assert_eq!(fields.text[0], "a0,1,2,3,4,5,6,7,");
    pending.append(RawInput { time: Some(43.0), events: vec![paste("new")], ..Default::default() });
    while !pending.events.is_empty() {
        fields.input_frame(pending.take_ordered_input());
    }
    assert_eq!(fields.text[0], "a0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,new");
}

#[test]
fn ipad_native_batches_preserve_event_order_and_one_shot_metadata() {
    let fields = Fields::new(false);
    let mut events = fields.tap(1);
    events.extend([Event::ModifiersChanged(Modifiers::SHIFT), paste("q"), Event::WindowFocused(false)]);
    events.extend(fields.tap(0));
    let original = events.clone();
    let mut pending = RawInput { time: Some(5.0), focused: false, events, ..Default::default() };
    pending.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().events.push(egui::ViewportEvent::Close);
    let mut delivered = Vec::new();
    let mut count = 0;
    while !pending.events.is_empty() {
        let batch = pending.take_ordered_input();
        assert!(!batch.events.is_empty());
        assert_eq!(batch.viewport().close_requested(), count == 0);
        assert_eq!(batch.time, Some(5.0));
        assert!(!batch.focused);
        delivered.extend(batch.events);
        count += 1;
        assert!(count <= original.len());
    }
    assert_eq!(delivered, original);
}

struct NativeEditor {
    ctx: Context,
    app: figcraft_ui_egui::FigApp,
    native: figcraft_ui_egui::touch_native::Native,
    field: String,
    field_rect: Rect,
    time: f64,
}

impl NativeEditor {
    fn new() -> Self {
        let mut editor = Self {
            ctx: Context::default(),
            app: figcraft_ui_egui::FigApp::new(figcraft_engine::Session::new(), Box::new(figcraft_ui_egui::NoServices)),
            native: figcraft_ui_egui::touch_native::Native::default(),
            field: String::new(),
            field_rect: Rect::NOTHING,
            time: 1.0,
        };
        editor.ctx.set_os(egui::os::OperatingSystem::IOS);
        editor.ctx.options_mut(|options| options.max_passes = std::num::NonZeroUsize::MIN);
        for _ in 0..3 {
            editor.redraw(&mut RawInput::default());
        }
        editor.app.run("tool.rectangle", serde_json::json!({})).unwrap();
        editor.app.session.prefs.keep_tool = true;
        editor
    }

    fn redraw(&mut self, pending: &mut RawInput) -> usize {
        self.time += 1.0 / 60.0;
        pending.time = Some(self.time);
        pending.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(1440.0, 900.0)));
        for count in 1..=RawInput::MAX_ORDERED_INPUT_BATCHES {
            let mut batch = pending.take_ordered_input();
            let frame = self.native.process(&mut self.app, &self.ctx, &mut batch);
            assert!(frame.issues.is_empty(), "{:?}", frame.issues);
            self.ctx
                .run_ui(batch, |ui| {
                    self.field_rect = ui.text_edit_singleline(&mut self.field).rect;
                    self.app.ui(ui);
                })
                .drop_without_applying_deltas();
            self.native.finish_frame(&self.app, &self.ctx, true);
            if pending.events.is_empty() {
                return count;
            }
        }
        RawInput::MAX_ORDERED_INPUT_BATCHES
    }

    fn run(&mut self, events: Vec<Event>) {
        let mut pending = RawInput { events, ..Default::default() };
        let count = self.redraw(&mut pending);
        assert!(pending.events.is_empty(), "input exceeded {count} batches");
    }

    fn node_count(&self) -> usize {
        self.app.session.doc().unwrap().nodes().count()
    }

    fn undo_count(&self) -> usize {
        self.app.session.ds().unwrap().history.undo.len()
    }
}

fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn mouse_drag(pos: Pos2) -> Vec<Event> {
    let end = pos + vec2(40.0, 30.0);
    vec![Event::PointerMoved(pos), pointer(pos, true), Event::PointerMoved(end), pointer(end, false)]
}

fn touch_event(pos: Pos2, phase: TouchPhase) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(1), phase, pos, force: None }
}

fn touch_drag(pos: Pos2) -> Vec<Event> {
    let end = pos + vec2(40.0, 30.0);
    vec![
        touch_event(pos, TouchPhase::Start),
        Event::PointerMoved(pos),
        pointer(pos, true),
        touch_event(end, TouchPhase::Move),
        Event::PointerMoved(end),
        touch_event(end, TouchPhase::End),
        pointer(end, false),
        Event::PointerGone,
    ]
}

#[test]
fn ipad_native_canvas_backlog_commits_each_drag_once_across_redraws() {
    let mut editor = NativeEditor::new();
    let nodes = editor.node_count();
    let undo = editor.undo_count();
    let pos = editor.app.canvas.rect.center();
    let mut pending = RawInput { events: (0..11).flat_map(|_| mouse_drag(pos)).collect(), ..Default::default() };
    assert_eq!(editor.redraw(&mut pending), RawInput::MAX_ORDERED_INPUT_BATCHES);
    assert!(!pending.events.is_empty());
    assert_eq!(editor.node_count(), nodes + 8);
    assert_eq!(editor.undo_count(), undo + 8);
    pending.append(RawInput { events: mouse_drag(pos), ..Default::default() });
    assert_eq!(editor.redraw(&mut pending), 4);
    assert!(pending.events.is_empty());
    assert_eq!(editor.node_count(), nodes + 12);
    assert_eq!(editor.undo_count(), undo + 12);
    assert!(editor.field.is_empty());
}

#[test]
fn ipad_native_canvas_then_field_paste_then_canvas_keeps_owners() {
    let mut editor = NativeEditor::new();
    let nodes = editor.node_count();
    let undo = editor.undo_count();
    let pos = editor.app.canvas.rect.center();
    let field = editor.field_rect.center();
    let mut events = mouse_drag(pos);
    events.extend([Event::PointerMoved(field), pointer(field, true), pointer(field, false), paste("synthetic")]);
    events.extend(mouse_drag(pos));
    editor.run(events);
    assert_eq!(editor.field, "synthetic");
    assert_eq!(editor.node_count(), nodes + 2);
    assert_eq!(editor.undo_count(), undo + 2);
}

#[test]
fn ipad_native_canvas_focus_loss_cancels_preview_and_allows_fresh_drag() {
    let mut editor = NativeEditor::new();
    let nodes = editor.node_count();
    let undo = editor.undo_count();
    let pos = editor.app.canvas.rect.center();
    editor.run(vec![Event::PointerMoved(pos), pointer(pos, true), Event::PointerMoved(pos + vec2(40.0, 30.0))]);
    assert!(editor.node_count() > nodes);
    let mut events = vec![Event::WindowFocused(false), pointer(pos, false), Event::WindowFocused(true)];
    events.extend(mouse_drag(pos));
    editor.run(events);
    assert_eq!(editor.node_count(), nodes + 1);
    assert_eq!(editor.undo_count(), undo + 1);
}

#[test]
fn ipad_native_canvas_touch_backlog_keeps_one_undo_per_gesture() {
    let mut editor = NativeEditor::new();
    let nodes = editor.node_count();
    let undo = editor.undo_count();
    let pos = editor.app.canvas.rect.center();
    let mut pending = RawInput { events: (0..10).flat_map(|_| touch_drag(pos)).collect(), ..Default::default() };
    assert_eq!(editor.redraw(&mut pending), RawInput::MAX_ORDERED_INPUT_BATCHES);
    assert!(!pending.events.is_empty());
    assert_eq!(editor.node_count(), nodes + 8);
    assert_eq!(editor.undo_count(), undo + 8);
    assert_eq!(editor.redraw(&mut pending), 2);
    assert!(pending.events.is_empty());
    assert_eq!(editor.node_count(), nodes + 10);
    assert_eq!(editor.undo_count(), undo + 10);
}

#[test]
fn ipad_native_canvas_checkpoint_cancel_preserves_prior_work_and_ignores_stale_release() {
    let mut editor = NativeEditor::new();
    let pos = editor.app.canvas.rect.center();
    editor.run(mouse_drag(pos));
    let before = editor.app.session.ds().unwrap().doc.clone();
    let nodes = editor.node_count();
    let undo = editor.undo_count();
    editor.run(vec![Event::PointerMoved(pos), pointer(pos, true), Event::PointerMoved(pos + vec2(40.0, 30.0))]);
    assert!(editor.node_count() > nodes);
    // The lifecycle callback posts this barrier even if the app is borrowed.
    figcraft_ui_egui::FigApp::request_canvas_pointer_cancel(&editor.ctx);
    editor.run(vec![pointer(pos, false)]);
    assert!(Arc::ptr_eq(&before, &editor.app.session.ds().unwrap().doc));
    assert_eq!(editor.undo_count(), undo);
    editor.run(mouse_drag(pos));
    assert_eq!(editor.node_count(), nodes + 1);
    assert_eq!(editor.undo_count(), undo + 1);
}

#[test]
fn ipad_unfocused_prefix_is_not_replayed_into_tapped_field() {
    for reverse in [false, true] {
        let mut fields = Fields::new(reverse);
        let mut events = vec![paste("ignored")];
        events.extend(fields.tap(1));
        events.push(paste("q"));
        fields.frame(events);
        assert_eq!(fields.text, ["a", "bq"]);
    }
}

#[test]
fn ipad_escape_after_tap_prevents_later_paste() {
    let mut fields = Fields::new(false);
    let mut events = fields.tap(1);
    events.extend([
        Event::Text("q".into()),
        Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE },
        paste("ignored"),
    ]);
    fields.frame(events);
    assert_eq!(fields.text, ["a", "bq"]);
    assert!(!fields.ctx.memory(|memory| memory.has_focus(Fields::id(1))));
}

#[test]
fn ipad_failed_owned_paste_preserves_selection_and_text() {
    let mut fields = Fields::new(false);
    fields.focus(0);
    let before = egui::TextEdit::load_state(&fields.ctx, Fields::id(0)).unwrap().cursor.char_range();
    fields.frame(vec![Event::OwnedPaste(egui::PasteOutcome::Failed(egui::PasteError::Denied))]);
    assert_eq!(fields.text, ["a", "b"]);
    assert_eq!(egui::TextEdit::load_state(&fields.ctx, Fields::id(0)).unwrap().cursor.char_range(), before);
}

#[test]
fn ipad_clipboard_callbacks_reject_cross_reentry_and_recover() {
    let ctx = Arc::new(Context::default());
    let weak = Arc::downgrade(&ctx);
    ctx.set_text_clipboard_writer(Some(Arc::new(move |_| {
        let ctx = weak.upgrade().unwrap();
        assert_eq!(ctx.read_owned_paste(), egui::PasteOutcome::Failed(egui::PasteError::Reentrant));
        Ok(())
    })));
    ctx.set_paste_reader(Some(Arc::new(|| egui::PasteOutcome::Empty)));
    assert_eq!(ctx.try_copy_text("synthetic".into()), Err(egui::TextClipboardError::Reentrant));
    ctx.set_text_clipboard_writer(Some(Arc::new(|_| Ok(()))));
    assert_eq!(ctx.try_copy_text("synthetic".into()), Ok(()));
    assert_eq!(ctx.read_owned_paste(), egui::PasteOutcome::Empty);
}

#[test]
fn ipad_clipboard_panics_and_invalid_receipts_do_not_poison_next_operation() {
    let ctx = Context::default();
    ctx.set_receipt_clipboard_writer(Some(Arc::new(|_| Ok(Some(egui::PasteIdentity { token: "invalid".into(), generation: 1 })))));
    assert_eq!(ctx.try_copy_text_receipt("synthetic".into()), Err(egui::TextClipboardError::InvalidContent));
    assert_eq!(ctx.take_text_clipboard_errors(), [egui::TextClipboardError::InvalidContent]);
    ctx.set_paste_reader(Some(Arc::new(|| panic!("synthetic callback failure"))));
    assert_eq!(ctx.read_owned_paste(), egui::PasteOutcome::Failed(egui::PasteError::Panicked));
    ctx.set_paste_reader(Some(Arc::new(|| egui::PasteOutcome::Empty)));
    assert_eq!(ctx.read_owned_paste(), egui::PasteOutcome::Empty);
}

#[test]
fn ipad_image_write_failure_reports_original_cause_and_recovers_after_reentry() {
    use egui::TextClipboardError as Error;
    let ctx = Arc::new(Context::default());
    let good = || egui::PasteImage { width: 1, height: 1, rgba: vec![0; 4] };
    assert_eq!(ctx.try_copy_rgba(good()), Err(Error::Unavailable));
    assert_eq!(ctx.try_copy_rgba(egui::PasteImage { width: 1, height: 1, rgba: vec![0; 3] }), Err(Error::InvalidContent));
    assert_eq!(ctx.try_copy_rgba(egui::PasteImage { width: 8193, height: 1, rgba: vec![] }), Err(Error::TooLarge));
    assert_eq!(ctx.take_text_clipboard_errors(), [Error::Unavailable, Error::InvalidContent, Error::TooLarge]);
    let weak = Arc::downgrade(&ctx);
    ctx.set_image_clipboard_writer(Some(Arc::new(move |_| {
        assert_eq!(weak.upgrade().unwrap().read_owned_paste(), egui::PasteOutcome::Failed(egui::PasteError::Reentrant));
        Err(Error::Denied)
    })));
    assert_eq!(ctx.try_copy_rgba(good()), Err(Error::Denied));
    assert_eq!(ctx.take_text_clipboard_errors(), [Error::Denied]);
    ctx.set_image_clipboard_writer(Some(Arc::new(|_| Ok(()))));
    assert_eq!(ctx.try_copy_rgba(good()), Ok(()));
    assert!(ctx.take_text_clipboard_errors().is_empty());
}

#[test]
fn ipad_owned_paste_reads_once_per_intent_without_output_fallback() {
    let ctx = Context::default();
    let reads = Arc::new(Mutex::new(0));
    let calls = reads.clone();
    ctx.set_paste_reader(Some(Arc::new(move || {
        let mut count = calls.lock().unwrap();
        *count += 1;
        egui::PasteOutcome::Content(egui::PasteContent { text: Some(count.to_string()), image: None, identity: None })
    })));
    let first = ctx.read_owned_paste();
    let second = ctx.read_owned_paste();
    assert_ne!(first, second);
    assert_eq!(*reads.lock().unwrap(), 2);
    ctx.set_paste_reader(None);
    assert!(ctx.owned_paste_required());
    assert_eq!(ctx.read_owned_paste(), egui::PasteOutcome::Failed(egui::PasteError::Unavailable));
    assert_eq!(*reads.lock().unwrap(), 2);
}

#[test]
fn ipad_invalid_copy_receipt_cannot_delete_selected_field_text() {
    let mut fields = Fields::new(false);
    fields.focus(0);
    fields.ctx.set_receipt_clipboard_writer(Some(Arc::new(|_| Ok(Some(egui::PasteIdentity { token: "invalid".into(), generation: 1 })))));
    fields.frame(vec![Event::Key {
        key: Key::A,
        physical_key: Some(Key::A),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::MAC_CMD | Modifiers::COMMAND,
    }]);
    let before = egui::TextEdit::load_state(&fields.ctx, Fields::id(0)).unwrap().cursor.char_range();
    fields.frame(vec![Event::Cut]);
    assert_eq!(fields.text, ["a", "b"]);
    assert_eq!(egui::TextEdit::load_state(&fields.ctx, Fields::id(0)).unwrap().cursor.char_range(), before);
    assert_eq!(fields.ctx.take_text_clipboard_errors(), [egui::TextClipboardError::InvalidContent]);
}

#[test]
fn ipad_cross_thread_reentry_refuses_success_without_deadlock() {
    let ctx = Context::default();
    let (entered, waiting) = std::sync::mpsc::channel();
    let (release, resume) = std::sync::mpsc::channel();
    let resume = Mutex::new(resume);
    ctx.set_text_clipboard_writer(Some(Arc::new(move |_| {
        entered.send(()).unwrap();
        resume.lock().unwrap().recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        Ok(())
    })));
    let worker_ctx = ctx.clone();
    let worker = std::thread::spawn(move || worker_ctx.try_copy_text("synthetic".into()));
    waiting.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    let nested = ctx.read_owned_paste();
    release.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(egui::TextClipboardError::Reentrant));
    assert_eq!(nested, egui::PasteOutcome::Failed(egui::PasteError::Reentrant));
    ctx.set_text_clipboard_writer(Some(Arc::new(|_| Ok(()))));
    assert_eq!(ctx.try_copy_text("synthetic".into()), Ok(()));
}
