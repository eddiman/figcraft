//! Specialized Native integration. A cloned pre-Down engine supplies the normal
//! external command outcome; cancellation must preserve that outcome exactly.
use super::combined_tests::Host;
use super::*;
use egui::{Event, Modifiers, PointerButton, TouchDeviceId, TouchId, TouchPhase, vec2};
use figcraft_doc::NodeId;
use figcraft_engine::{
    Session,
    tools::{PointerEvent, PointerKind},
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Cause {
    Second,
    Cancel,
    Focus,
    Allocation,
    Viewport,
    Modal,
    Switch,
    Close,
    Unavailable,
    Panic,
}
const CAUSES: [Cause; 10] = [
    Cause::Second,
    Cause::Cancel,
    Cause::Focus,
    Cause::Allocation,
    Cause::Viewport,
    Cause::Modal,
    Cause::Switch,
    Cause::Close,
    Cause::Unavailable,
    Cause::Panic,
];
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fixture {
    Prototype,
    Pen,
    Text,
    VectorBend,
    VectorHandle,
    CommentPin,
    Move,
    Duplicate,
    Resize,
    Rotate,
    Arc,
    Radius,
    VectorPoint,
    VectorWidth,
    VectorRegion,
    Freehand,
    NewText,
    OutsideText,
    CommentCluster,
    GradientEnd,
    GradientStop,
    GradientWidth,
    CropMove,
    CropScale,
    BlurStart,
    BlurEnd,
    Padding,
    Gap,
    CrossGap,
    GuideCreate,
    GuideMove,
    GuideCopy,
    GuideDelete,
    Draw(&'static str),
    Marquee,
    Hand,
    Scale,
}
fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch { device_id: TouchDeviceId(1), id: TouchId(id), phase, pos, force: None }
}
fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}
fn id(value: serde_json::Value) -> NodeId {
    NodeId(value["id"].as_u64().unwrap())
}
fn seed(fixture: Fixture, cause: Cause) -> (Host, Pos2, Pos2) {
    let mut h = Host::new();
    h.ui.app.run("tool.move", json!({})).unwrap();
    // Create the destination before any multi-click pen/text interaction exists.
    if cause == Cause::Switch {
        h.ui.app.run("file.new", json!({})).unwrap();
        h.run(vec![]);
        h.ui.app.run("doc.switch", json!({"index":0})).unwrap();
        h.run(vec![]);
    }
    {
        let view = &mut h.ui.app.session.ds_mut().unwrap().view;
        view.cx = 0.0;
        view.cy = 0.0;
    }
    let (from, to) = match fixture {
        Fixture::Prototype => {
            let source = id(h.ui.app.run("shape.rectangle", json!({"x":-150,"y":-50,"width":100,"height":100})).unwrap());
            h.ui.app.run("frame.create", json!({"x":100,"y":-50,"width":120,"height":100})).unwrap();
            h.ui.app.run("select.set", json!({"ids":[source]})).unwrap();
            h.ui.app.ui.right_tab = crate::RightTab::Prototype;
            (h.ui.app.to_screen(-50.0, 0.0) + vec2(11.0, 0.0), h.ui.app.to_screen(160.0, 0.0))
        }
        Fixture::Pen => {
            h.ui.app.run("tool.pen", json!({})).unwrap();
            h.run(vec![]);
            for point in [(-150.0, -60.0), (-80.0, -60.0)] {
                h.pointer = h.ui.app.to_screen(point.0, point.1);
                h.start();
                h.end();
            }
            assert_eq!(h.ui.app.session.tool.pen.as_ref().unwrap().points.len(), 2);
            (h.ui.app.to_screen(0.0, 0.0), h.ui.app.to_screen(30.0, 40.0))
        }
        Fixture::Text | Fixture::OutsideText => {
            let text = id(h.ui.app.run("text.create", json!({"text":"Before","x":-150,"y":-30,"width":300,"props":{"fontSize":24}})).unwrap());
            figcraft_engine::textedit::begin(&mut h.ui.app.session, text, Some(6), false).unwrap();
            h.run(vec![Event::Text("A".into()), Event::Ime(egui::ImeEvent::Preedit { text: "Synthetic".into(), active_range_chars: Some(2..2) })]);
            assert!(h.ui.app.session.ds().unwrap().text_edit.as_ref().unwrap().preedit.is_some());
            if fixture == Fixture::OutsideText {
                (h.ui.app.to_screen(120.0, 80.0), h.ui.app.to_screen(170.0, 120.0))
            } else {
                (h.ui.app.to_screen(-145.0, -18.0), h.ui.app.to_screen(-80.0, -18.0))
            }
        }
        Fixture::VectorBend | Fixture::VectorHandle => {
            let shape = id(h.ui.app.run("shape.rectangle", json!({"x":-50,"y":-50,"width":100,"height":100})).unwrap());
            h.ui.app.run("vector.edit.begin", json!({"id":shape})).unwrap();
            h.ui.app.run("vector.edit.tool", json!({"tool":"bend"})).unwrap();
            if fixture == Fixture::VectorHandle {
                for (kind, x, y) in [(PointerKind::Down, 50.0, -50.0), (PointerKind::Move, 80.0, -50.0), (PointerKind::Up, 80.0, -50.0)] {
                    h.ui.app.session.pointer(&PointerEvent::at(kind, x, y, 1.0)).unwrap();
                }
                h.ui.app.run("vector.edit.tool", json!({"tool":"move"})).unwrap();
                let ds = h.ui.app.session.ds().unwrap();
                let overlay = figcraft_engine::vectoredit::overlay(&ds.doc, ds.vector_edit.as_ref().unwrap()).unwrap();
                let p = overlay.handles.first().unwrap().handle;
                (h.ui.app.to_screen(p[0], p[1]), h.ui.app.to_screen(p[0] + 30.0, p[1] + 30.0))
            } else {
                (h.ui.app.to_screen(50.0, -50.0), h.ui.app.to_screen(80.0, -50.0))
            }
        }
        Fixture::Move | Fixture::Duplicate | Fixture::Resize | Fixture::Rotate | Fixture::Radius | Fixture::Arc | Fixture::Scale => {
            let command = if fixture == Fixture::Arc { "shape.ellipse" } else { "shape.rectangle" };
            h.ui.app.run(command, json!({"x":-60,"y":-50,"width":120,"height":100})).unwrap();
            if fixture == Fixture::Scale {
                h.ui.app.run("tool.scale", json!({})).unwrap();
            }
            h.run(vec![]);
            let from = match fixture {
                Fixture::Resize | Fixture::Scale => h.ui.app.to_screen(60.0, 50.0),
                Fixture::Rotate => h.ui.app.to_screen(-70.0, -60.0),
                Fixture::Arc => {
                    let ds = h.ui.app.session.ds().unwrap();
                    let p = figcraft_engine::arc_handles::handles(&ds.doc, &ds.selection, ds.view.zoom).unwrap().sweep;
                    h.ui.app.to_screen(p[0], p[1])
                }
                Fixture::Radius => {
                    let ds = h.ui.app.session.ds().unwrap();
                    let (_, p) = figcraft_engine::shape_handles::handles(&ds.doc, &ds.selection, ds.view.zoom).first().copied().unwrap();
                    h.ui.app.to_screen(p[0], p[1])
                }
                _ => h.ui.app.to_screen(0.0, 0.0),
            };
            let to = match fixture {
                Fixture::Rotate | Fixture::Arc => h.ui.app.to_screen(0.0, -110.0),
                _ => from + vec2(35.0, 28.0),
            };
            if fixture == Fixture::Duplicate {
                h.run(vec![Event::ModifiersChanged(Modifiers { alt: true, ..Default::default() })]);
            }
            (from, to)
        }
        Fixture::VectorPoint | Fixture::VectorWidth | Fixture::VectorRegion => {
            let shape = id(h.ui.app.run("shape.rectangle", json!({"x":-60,"y":-50,"width":120,"height":100,"props":{"strokeWeight":10}})).unwrap());
            h.ui.app.run("vector.edit.begin", json!({"id":shape})).unwrap();
            h.ui.app.run("vector.edit.tool", json!({"tool":if fixture == Fixture::VectorWidth { "width" } else { "move" }})).unwrap();
            let from = match fixture {
                Fixture::VectorWidth => h.ui.app.to_screen(0.0, -50.0),
                Fixture::VectorRegion => {
                    h.ui.app.run("vector.edit.select", json!({"regions":[0]})).unwrap();
                    h.ui.app.to_screen(0.0, 0.0)
                }
                _ => h.ui.app.to_screen(-60.0, -50.0),
            };
            (from, from + vec2(35.0, 28.0))
        }
        Fixture::Freehand => {
            h.ui.app.session.prefs.keep_tool = true;
            h.ui.app.run("tool.pencil", json!({})).unwrap();
            h.run(vec![]);
            h.pointer = h.ui.app.to_screen(-140.0, -90.0);
            h.start();
            h.moved(h.ui.app.to_screen(-90.0, -65.0));
            h.end();
            assert_eq!(h.ui.app.session.doc().unwrap().nodes().count(), 2);
            (h.ui.app.to_screen(0.0, 0.0), h.ui.app.to_screen(60.0, 40.0))
        }
        Fixture::NewText => {
            h.ui.app.run("tool.text", json!({})).unwrap();
            (h.ui.app.to_screen(-100.0, -60.0), h.ui.app.to_screen(80.0, 20.0))
        }
        Fixture::CommentCluster => {
            h.ui.app.run("tool.comment", json!({})).unwrap();
            for x in [0.0, 8.0] {
                h.ui.app.run("comment.add", json!({"x":x,"y":0,"text":"Synthetic clustered pin"})).unwrap();
            }
            h.ui.app.ui.comments.close();
            let from = h.ui.app.to_screen(4.0, 0.0) + vec2(14.0, -14.0);
            (from, from + vec2(45.0, 30.0))
        }
        Fixture::GradientEnd | Fixture::GradientStop | Fixture::GradientWidth => {
            let shape = id(h.ui.app.run("shape.rectangle", json!({"x":-90,"y":-80,"width":180,"height":160})).unwrap());
            h.ui.app.run("node.set", json!({"ids":[shape],"fills":[{"type":if fixture == Fixture::GradientWidth { "radialGradient" } else { "linearGradient" },"stops":["#ff0000","#00ff00","#0000ff"],"from":[0.2,0.3],"to":[0.8,0.3],"width":[0.2,0.9]}]})).unwrap();
            h.ui.app.ui.picker = Some(crate::colorpicker::PickerTarget::Paint { ids: vec![shape], fills: true, index: 0 });
            h.run(vec![]);
            let which = match fixture {
                Fixture::GradientStop => 0,
                Fixture::GradientEnd => 1,
                _ => 2,
            };
            let from = crate::handles::native_fixture::gradient(&h.ui.app, which).unwrap();
            (from, from + vec2(24.0, 20.0))
        }
        Fixture::CropMove | Fixture::CropScale => {
            use base64::Engine as _;
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(4, 4, image::Rgba([160, 70, 30, 255])))
                .write_to(&mut bytes, image::ImageFormat::Png)
                .unwrap();
            let data = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
            let shape = id(h.ui.app.run("image.place", json!({"data":data,"x":-90,"y":-80,"width":180,"height":160})).unwrap());
            let mut paint = h.ui.app.session.doc().unwrap().node(shape).unwrap().fills[0].clone();
            let figcraft_color::PaintKind::Image(image) = &mut paint.kind else { panic!("synthetic image paint") };
            image.scale_mode = figcraft_color::ImageScaleMode::Crop;
            h.ui.app.run("fill.set", json!({"ids":[shape],"index":0,"paint":paint})).unwrap();
            h.ui.app.ui.picker = Some(crate::colorpicker::PickerTarget::Paint { ids: vec![shape], fills: true, index: 0 });
            h.run(vec![]);
            let from = crate::handles::native_fixture::crop(&h.ui.app, fixture == Fixture::CropScale).unwrap();
            (from, from + vec2(30.0, 25.0))
        }
        Fixture::BlurStart | Fixture::BlurEnd => {
            let shape = id(h.ui.app.run("shape.rectangle", json!({"x":-90,"y":-80,"width":180,"height":160})).unwrap());
            h.ui.app.run("effect.add", json!({"ids":[shape],"effect":{"type":"layerBlur","radius":20,"blurType":"PROGRESSIVE","startRadius":2,"startOffset":[0.2,0.3],"endOffset":[0.8,0.7]}})).unwrap();
            h.ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new(("fx_open", 0usize)), true));
            h.run(vec![]);
            let from = crate::handles::native_fixture::blur(&h.ui.app, &h.ui.ctx, usize::from(fixture == Fixture::BlurEnd)).unwrap();
            (from, from + vec2(28.0, 20.0))
        }
        Fixture::Padding | Fixture::Gap | Fixture::CrossGap => {
            let shape = id(h.ui.app.run("frame.create", json!({"x":-150,"y":-120,"width":300,"height":240,"props":{"layout":"horizontal","padding":20,"gap":20,"crossGap":30,"wrap":fixture == Fixture::CrossGap}})).unwrap());
            for _ in 0..3 {
                h.ui.app.run("shape.rectangle", json!({"parent":shape,"width":120,"height":60})).unwrap();
            }
            h.ui.app.run("select.set", json!({"ids":[shape]})).unwrap();
            h.run(vec![]);
            let from = crate::handles::native_fixture::layout(&h.ui.app, fixture == Fixture::Padding, fixture == Fixture::CrossGap).unwrap();
            (from, from + vec2(25.0, 25.0))
        }
        Fixture::GuideCreate | Fixture::GuideMove | Fixture::GuideCopy | Fixture::GuideDelete => {
            h.ui.app.session.ds_mut().unwrap().view.rulers = true;
            if fixture != Fixture::GuideCreate {
                h.ui.app.run("guide.add", json!({"vertical":true,"offset":0})).unwrap();
            }
            h.run(vec![]);
            if fixture == Fixture::GuideCopy {
                h.run(vec![Event::ModifiersChanged(Modifiers { alt: true, ..Default::default() })]);
            }
            let ruler = Pos2::new(h.ui.app.canvas.rect.min.x + 10.0, h.ui.app.canvas.rect.center().y);
            let from = if fixture == Fixture::GuideCreate { ruler } else { h.ui.app.to_screen(0.0, 0.0) };
            let to = if fixture == Fixture::GuideDelete { ruler } else { h.ui.app.to_screen(65.0, 30.0) };
            (from, to)
        }
        Fixture::Draw(tool) => {
            h.ui.app.run(&format!("tool.{tool}"), json!({})).unwrap();
            (h.ui.app.to_screen(-120.0, -80.0), h.ui.app.to_screen(20.0, 20.0))
        }
        Fixture::Marquee => {
            let first = id(h.ui.app.run("shape.rectangle", json!({"x":-60,"y":-40,"width":40,"height":40})).unwrap());
            h.ui.app.run("shape.rectangle", json!({"x":0,"y":0,"width":40,"height":40})).unwrap();
            h.ui.app.run("select.set", json!({"ids":[first]})).unwrap();
            (h.ui.app.to_screen(-120.0, -80.0), h.ui.app.to_screen(80.0, 70.0))
        }
        Fixture::Hand => {
            h.ui.app.run("tool.hand", json!({})).unwrap();
            (h.ui.app.to_screen(-120.0, -80.0), h.ui.app.to_screen(20.0, 20.0))
        }
        Fixture::CommentPin => {
            h.ui.app.run("tool.comment", json!({})).unwrap();
            h.ui.app.run("comment.add", json!({"x":0,"y":0,"text":"Synthetic matrix pin"})).unwrap();
            let p = h.ui.app.to_screen(0.0, 0.0) + vec2(14.0, -14.0);
            (p, p + vec2(70.0, 50.0))
        }
    };
    h.run(vec![]);
    h.run(vec![]);
    h.run(vec![]);
    let (from, to) = if fixture == Fixture::VectorPoint {
        assert!(h.ui.app.session.ds().unwrap().vector_edit.as_ref().unwrap().vertices.is_empty());
        let from = h.ui.app.to_screen(-60.0, -50.0);
        (from, from + vec2(35.0, 28.0))
    } else if matches!(fixture, Fixture::Draw(_) | Fixture::Hand | Fixture::Marquee) {
        (
            h.ui.app.to_screen(-120.0, -80.0),
            h.ui.app.to_screen(if fixture == Fixture::Marquee { 80.0 } else { 20.0 }, if fixture == Fixture::Marquee { 70.0 } else { 20.0 }),
        )
    } else {
        (from, to)
    };
    h.pointer = from;
    (h, from, to)
}
fn control(h: &Host) -> Session {
    let mut s = Session::empty();
    s.docs = h.ui.app.session.docs.clone();
    s.active = h.ui.app.session.active;
    s.tool = h.ui.app.session.tool.clone();
    s.prefs = h.ui.app.session.prefs.clone();
    s
}
fn interrupt(h: &mut Host, expected: &mut Session, cause: Cause) {
    match cause {
        Cause::Second => h.run(vec![touch(2, TouchPhase::Start, h.pointer + vec2(60.0, 20.0))]),
        Cause::Cancel => h.run(vec![touch(1, TouchPhase::Cancel, h.pointer), Event::PointerGone]),
        Cause::Focus => h.run(vec![Event::WindowFocused(false)]),
        Cause::Allocation => {
            h.ui.app.canvas.rect.max.x -= 20.0;
            h.run(vec![]);
        }
        Cause::Viewport => h.run_custom(vec![], &[], |raw| raw.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(900.0, 1440.0)))),
        Cause::Modal | Cause::Switch | Cause::Close => {
            let (command, params) = match cause {
                Cause::Modal => ("help.about", json!({})),
                Cause::Switch => ("doc.switch", json!({"index":1})),
                _ => ("file.close", json!({"force":true})),
            };
            expected.execute(command, &params).unwrap();
            h.ui.app.run(command, params).unwrap();
            h.run(vec![]);
        }
        Cause::Panic => {
            let completed = crate::windows::guard(&mut h.ui.app, &h.ui.ctx, |_| panic!("synthetic matrix UI panic"));
            assert!(!completed);
            h.native.finish_frame(&h.ui.app, &h.ui.ctx, completed);
            assert!(h.native.consumer.layout.is_none());
            h.run(vec![]);
        }
        Cause::Unavailable => {
            h.unavailable(vec![]);
            h.run(vec![]);
        }
    }
}
fn assert_engine(actual: &Session, expected: &Session, fixture: Fixture, cause: Cause) {
    assert_eq!(actual.docs.len(), expected.docs.len(), "{fixture:?}/{cause:?}");
    assert_eq!(actual.active, expected.active, "{fixture:?}/{cause:?}: active document");
    for e in &expected.docs {
        let a = actual.docs.iter().find(|a| a.uid == e.uid).unwrap();
        assert_eq!(*a.doc, *e.doc, "{fixture:?}/{cause:?}: document");
        assert_eq!(a.selection, e.selection, "{fixture:?}/{cause:?}: selection");
        assert_eq!(a.page, e.page, "{fixture:?}/{cause:?}: page");
        assert_eq!(a.hover, e.hover, "{fixture:?}/{cause:?}: hover");
        assert_eq!(a.guide, e.guide, "{fixture:?}/{cause:?}: guide selection");
        assert_eq!(a.focus, e.focus, "{fixture:?}/{cause:?}: focus");
        assert_eq!(a.text_preview, e.text_preview, "{fixture:?}/{cause:?}: text preview");
        assert_eq!(a.moved_origin(), e.moved_origin(), "{fixture:?}/{cause:?}: rotation origin");
        assert_eq!(a.text_edit, e.text_edit, "{fixture:?}/{cause:?}: text/preedit");
        assert_eq!(a.vector_edit, e.vector_edit, "{fixture:?}/{cause:?}: vector sub-selection");
        assert_eq!((a.history.undo.len(), a.history.redo.len()), (e.history.undo.len(), e.history.redo.len()), "{fixture:?}/{cause:?}: history");
        assert_eq!(a.history.limit, e.history.limit, "{fixture:?}/{cause:?}: history limit");
        for (a, e) in a.history.undo.iter().chain(&a.history.redo).zip(e.history.undo.iter().chain(&e.history.redo)) {
            assert_eq!((&a.label, &a.selection, a.page), (&e.label, &e.selection, e.page), "{fixture:?}/{cause:?}: history entry");
            assert_eq!(*a.doc, *e.doc, "{fixture:?}/{cause:?}: history document");
        }
        assert_eq!((a.view.cx, a.view.cy, a.view.zoom), (e.view.cx, e.view.cy, e.view.zoom), "{fixture:?}/{cause:?}: view");
        match (&a.interaction, &e.interaction) {
            (Some(a), Some(e)) => assert!(Arc::ptr_eq(&a.doc, &e.doc), "{fixture:?}/{cause:?}: original interaction"),
            (None, None) => {}
            _ => panic!("{fixture:?}/{cause:?}: interaction changed"),
        }
    }
    assert_eq!(actual.tool.tool, expected.tool.tool, "{fixture:?}/{cause:?}: tool");
    assert_eq!(
        actual.tool.pen.as_ref().map(|p| (&p.id, &p.points, p.closed, p.join)),
        expected.tool.pen.as_ref().map(|p| (&p.id, &p.points, p.closed, p.join)),
        "{fixture:?}/{cause:?}: accepted pen path"
    );
    assert!(!actual.tool.busy(), "{fixture:?}/{cause:?}: no finished drag remains");
}
fn handle_owner(fixture: Fixture) -> Option<&'static str> {
    match fixture {
        Fixture::GradientEnd | Fixture::GradientStop | Fixture::GradientWidth => Some("gradient"),
        Fixture::CropMove | Fixture::CropScale => Some("crop"),
        Fixture::BlurStart | Fixture::BlurEnd => Some("blur"),
        Fixture::Padding => Some("padding"),
        Fixture::Gap | Fixture::CrossGap => Some("gap"),
        _ => None,
    }
}
fn guide_owner(fixture: Fixture) -> Option<&'static str> {
    match fixture {
        Fixture::GuideCreate => Some("new"),
        Fixture::GuideCopy => Some("copy"),
        Fixture::GuideMove | Fixture::GuideDelete => Some("move"),
        _ => None,
    }
}
fn normal_completion(fixture: Fixture) {
    if matches!(fixture, Fixture::Prototype | Fixture::Pen | Fixture::Text | Fixture::VectorBend | Fixture::VectorHandle | Fixture::CommentPin) {
        return;
    }
    let (mut h, from, to) = seed(fixture, Cause::Second);
    let before = h.ui.app.session.ds().unwrap().doc.clone();
    let source_view = &h.ui.app.session.ds().unwrap().view;
    let view = (source_view.cx, source_view.cy, source_view.zoom);
    let allocation = h.ui.app.canvas.rect;
    h.pointer = from;
    h.start();
    h.moved(to);
    h.end();
    assert!(h.ui.app.canvas_pointer.is_none());
    let ds = h.ui.app.session.ds().unwrap();
    if fixture == Fixture::VectorPoint {
        assert_eq!(h.ui.app.canvas.rect, allocation, "point selection keeps the canvas allocation");
        assert_eq!(ds.vector_edit.as_ref().unwrap().vertices, vec![0], "accepted drag selected the initially unselected vertex");
    }
    if fixture == Fixture::OutsideText {
        assert!(ds.text_edit.is_none(), "accepted outside click ended the earlier text edit");
    } else if fixture == Fixture::Marquee {
        assert!(Arc::ptr_eq(&ds.doc, &before));
        assert_eq!(ds.selection.len(), 2, "accepted marquee selected both layers");
    } else if fixture == Fixture::CommentCluster || fixture == Fixture::Hand {
        assert_ne!((ds.view.cx, ds.view.cy, ds.view.zoom), view, "{fixture:?}: accepted view change");
        assert!(Arc::ptr_eq(&ds.doc, &before));
    } else {
        assert_ne!(*ds.doc, *before, "{fixture:?}: normal release changes the document");
        if guide_owner(fixture).is_some() {
            let guides = ds.doc.node(ds.page).unwrap().guides();
            let count = match fixture {
                Fixture::GuideCreate | Fixture::GuideMove => 1,
                Fixture::GuideCopy => 2,
                _ => 0,
            };
            assert_eq!(guides.len(), count, "{fixture:?}: accepted guide operation");
            let offsets: Vec<_> = guides.iter().map(|g| (g.vertical, g.offset)).collect();
            let expected = match fixture {
                Fixture::GuideCreate | Fixture::GuideMove => vec![(true, 65.0)],
                Fixture::GuideCopy => vec![(true, 0.0), (true, 65.0)],
                _ => vec![],
            };
            assert_eq!(offsets, expected, "{fixture:?}: accepted guide destinations");
        }
        if fixture == Fixture::Duplicate {
            assert_eq!(ds.doc.nodes().count(), before.nodes().count() + 1, "accepted Alt drag duplicates");
        }
        if fixture == Fixture::Freehand {
            assert_eq!(ds.doc.nodes().count(), before.nodes().count() + 1, "accepted second stroke retains first");
        }
    }
}
fn matrix(fixture: Fixture) {
    normal_completion(fixture);
    for cause in CAUSES {
        let (mut h, from, to) = seed(fixture, cause);
        let mut expected = control(&h);
        let before = h.ui.app.session.ds().unwrap().doc.clone();
        h.pointer = from;
        h.start();
        h.moved(to);
        assert!(
            h.ui.app.canvas_pointer.is_some(),
            "{fixture:?}/{cause:?}: exercise actual enrolled preview; phase={:?}, hit={:?}, tool={:?}, toast={:?}",
            h.native.phase(),
            h.native.consumer.layout.as_ref().map(|layout| layout.classify(&h.ui.ctx, from)),
            h.ui.app.session.tool.tool,
            h.ui.app.ui.toast
        );
        if let Some(owner) = handle_owner(fixture) {
            assert_eq!(crate::handles::native_fixture::owner(&h.ui.ctx), Some(owner), "{fixture:?}/{cause:?}: actual handle owner");
            assert_ne!(*before, *h.ui.app.session.ds().unwrap().doc, "{fixture:?}/{cause:?}: actual handle geometry");
        } else if let Some(owner) = guide_owner(fixture) {
            assert_eq!(crate::guides::native_pointer_preview(&h.ui.ctx), Some((owner, true)), "{fixture:?}/{cause:?}: actual guide preview");
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc), "guide edits only commit on release");
        } else if matches!(
            fixture,
            Fixture::Pen
                | Fixture::VectorBend
                | Fixture::VectorHandle
                | Fixture::Move
                | Fixture::Duplicate
                | Fixture::Resize
                | Fixture::Scale
                | Fixture::Rotate
                | Fixture::Arc
                | Fixture::Radius
                | Fixture::VectorPoint
                | Fixture::VectorWidth
                | Fixture::VectorRegion
        ) {
            assert_ne!(*before, *h.ui.app.session.ds().unwrap().doc, "{fixture:?}/{cause:?}: actual geometry preview");
        } else if fixture == Fixture::Freehand || matches!(fixture, Fixture::Draw(_)) {
            assert!(h.ui.app.session.tool.busy(), "{fixture:?}/{cause:?}: actual provisional stroke");
        } else if fixture == Fixture::Marquee {
            assert!(h.ui.app.session.tool.overlay.marquee.is_some());
            assert_ne!(h.ui.app.session.selection(), expected.selection(), "actual marquee selection preview");
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        } else if fixture == Fixture::Hand {
            let view = &h.ui.app.session.ds().unwrap().view;
            let old = &expected.ds().unwrap().view;
            assert_ne!((view.cx, view.cy), (old.cx, old.cy), "actual Hand pan preview");
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        } else if fixture == Fixture::NewText {
            assert_ne!(*before, *h.ui.app.session.ds().unwrap().doc, "new text Down created its provisional layer");
            assert!(h.ui.app.session.ds().unwrap().text_edit.is_some());
        } else if fixture == Fixture::OutsideText {
            assert!(h.ui.app.session.ds().unwrap().text_edit.is_none(), "outside Down ended the old edit within the checkpoint");
        } else if fixture == Fixture::CommentCluster {
            assert_ne!(h.ui.app.session.ds().unwrap().view.zoom, expected.ds().unwrap().view.zoom, "cluster Down changed the view");
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc));
        } else if fixture == Fixture::CommentPin {
            assert!(h.ui.app.ui.comments.dragging());
        } else if fixture == Fixture::Prototype {
            assert!(Arc::ptr_eq(&before, &h.ui.app.session.ds().unwrap().doc), "prototype preview is UI only");
        }
        if fixture == Fixture::VectorPoint {
            assert_eq!(h.ui.app.session.ds().unwrap().vector_edit.as_ref().unwrap().vertices, vec![0], "actual point selection, not a region drag");
        }
        if fixture == Fixture::VectorRegion {
            assert_eq!(h.ui.app.session.ds().unwrap().vector_edit.as_ref().unwrap().regions, vec![0], "actual region drag");
        }
        if fixture == Fixture::Arc {
            assert!(h.ui.app.session.tool.overlay.arc.is_some());
        }
        if fixture == Fixture::Radius {
            assert!(h.ui.app.session.tool.overlay.shape.is_some());
        }
        interrupt(&mut h, &mut expected, cause);
        assert!(h.ui.app.canvas_pointer.is_none(), "{fixture:?}/{cause:?}: owner released");
        assert_engine(&h.ui.app.session, &expected, fixture, cause);
        if cause == Cause::Focus {
            h.run_checked(
                vec![touch(1, TouchPhase::End, h.pointer), button(h.pointer, false), Event::PointerGone],
                &[crate::touch_events::Issue::MissingStart, crate::touch_events::Issue::PhysicalInput],
            );
        } else if cause == Cause::Cancel {
            h.run(vec![button(h.pointer, false)]);
        } else {
            h.end();
        }
        if cause == Cause::Second {
            h.run(vec![touch(2, TouchPhase::End, to + vec2(60.0, 20.0)), Event::PointerGone]);
        }
        assert_engine(&h.ui.app.session, &expected, fixture, cause);
        assert!(!h.ui.app.ui.comments.dragging());
        if fixture == Fixture::Marquee {
            assert!(h.ui.app.session.tool.overlay.marquee.is_none());
        }
        assert!(crate::handles::native_fixture::owner(&h.ui.ctx).is_none());
        assert!(crate::guides::native_pointer_preview(&h.ui.ctx).is_none());
    }
}
#[test]
fn ipad_native_matrix_prototype_connector() {
    matrix(Fixture::Prototype);
}
#[test]
fn ipad_native_matrix_prior_pen_anchors() {
    matrix(Fixture::Pen);
}
#[test]
fn ipad_native_matrix_prior_text_and_preedit() {
    matrix(Fixture::Text);
}
#[test]
fn ipad_native_matrix_vector_bend() {
    matrix(Fixture::VectorBend);
}
#[test]
fn ipad_native_matrix_vector_bezier_handle() {
    matrix(Fixture::VectorHandle);
}
#[test]
fn ipad_native_matrix_comment_pin() {
    matrix(Fixture::CommentPin);
}

#[test]
fn ipad_native_matrix_move() {
    matrix(Fixture::Move);
}

#[test]
fn ipad_native_matrix_duplicate() {
    matrix(Fixture::Duplicate);
}

#[test]
fn ipad_native_matrix_resize() {
    matrix(Fixture::Resize);
}

#[test]
fn ipad_native_matrix_rotate() {
    matrix(Fixture::Rotate);
}

#[test]
fn ipad_native_matrix_arc() {
    matrix(Fixture::Arc);
}

#[test]
fn ipad_native_matrix_radius() {
    matrix(Fixture::Radius);
}

#[test]
fn ipad_native_matrix_vector_point() {
    matrix(Fixture::VectorPoint);
}

#[test]
fn ipad_native_matrix_vector_width() {
    matrix(Fixture::VectorWidth);
}

#[test]
fn ipad_native_matrix_vector_region() {
    matrix(Fixture::VectorRegion);
}

#[test]
fn ipad_native_matrix_prior_freehand() {
    matrix(Fixture::Freehand);
}

#[test]
fn ipad_native_matrix_new_text() {
    matrix(Fixture::NewText);
}

#[test]
fn ipad_native_matrix_outside_text() {
    matrix(Fixture::OutsideText);
}

#[test]
fn ipad_native_matrix_comment_cluster() {
    matrix(Fixture::CommentCluster);
}

#[test]
fn ipad_native_matrix_gradient_end() {
    matrix(Fixture::GradientEnd);
}

#[test]
fn ipad_native_matrix_gradient_stop() {
    matrix(Fixture::GradientStop);
}

#[test]
fn ipad_native_matrix_gradient_width() {
    matrix(Fixture::GradientWidth);
}

#[test]
fn ipad_native_matrix_crop_move() {
    matrix(Fixture::CropMove);
}

#[test]
fn ipad_native_matrix_crop_scale() {
    matrix(Fixture::CropScale);
}

#[test]
fn ipad_native_matrix_blur_start() {
    matrix(Fixture::BlurStart);
}

#[test]
fn ipad_native_matrix_blur_end() {
    matrix(Fixture::BlurEnd);
}

#[test]
fn ipad_native_matrix_padding() {
    matrix(Fixture::Padding);
}

#[test]
fn ipad_native_matrix_gap() {
    matrix(Fixture::Gap);
}

#[test]
fn ipad_native_matrix_cross_gap() {
    matrix(Fixture::CrossGap);
}

#[test]
fn ipad_native_matrix_guide_create() {
    matrix(Fixture::GuideCreate);
}

#[test]
fn ipad_native_matrix_guide_move() {
    matrix(Fixture::GuideMove);
}

#[test]
fn ipad_native_matrix_guide_copy() {
    matrix(Fixture::GuideCopy);
}

#[test]
fn ipad_native_matrix_guide_delete() {
    matrix(Fixture::GuideDelete);
}

#[test]
fn ipad_native_matrix_draw_rectangle() {
    matrix(Fixture::Draw("rectangle"));
}

#[test]
fn ipad_native_matrix_draw_ellipse() {
    matrix(Fixture::Draw("ellipse"));
}

#[test]
fn ipad_native_matrix_draw_frame() {
    matrix(Fixture::Draw("frame"));
}

#[test]
fn ipad_native_matrix_draw_section() {
    matrix(Fixture::Draw("section"));
}

#[test]
fn ipad_native_matrix_draw_slice() {
    matrix(Fixture::Draw("slice"));
}

#[test]
fn ipad_native_matrix_draw_line() {
    matrix(Fixture::Draw("line"));
}

#[test]
fn ipad_native_matrix_draw_arrow() {
    matrix(Fixture::Draw("arrow"));
}

#[test]
fn ipad_native_matrix_draw_polygon() {
    matrix(Fixture::Draw("polygon"));
}

#[test]
fn ipad_native_matrix_draw_star() {
    matrix(Fixture::Draw("star"));
}

#[test]
fn ipad_native_matrix_marquee() {
    matrix(Fixture::Marquee);
}

#[test]
fn ipad_native_matrix_hand() {
    matrix(Fixture::Hand);
}

#[test]
fn ipad_native_matrix_scale() {
    matrix(Fixture::Scale);
}

fn ui_control(h: &Host) -> Host {
    let mut c = Host::new();
    c.ui.app.session = control(h);
    c.ui.app.ui.home = h.ui.app.ui.home;
    c.ui.app.ui.right_tab = h.ui.app.ui.right_tab;
    c.ui.app.ui.comments.restore_pointer_ui(h.ui.app.ui.comments.pointer_ui());
    c.ui.app.ui.comments.search.clone_from(&h.ui.app.ui.comments.search);
    c.ui.app.ui.comments.filter = h.ui.app.ui.comments.filter;
    c.ui.app.ui.comments.sort = h.ui.app.ui.comments.sort;
    crate::proto_panel::open_editor(&c.ui.ctx, crate::proto_panel::editor(&h.ui.ctx));
    c.time = h.time;
    for _ in 0..3 {
        c.run(vec![]);
    }
    c
}
fn ui_control_interrupt(c: &mut Host, cause: Cause) {
    match cause {
        Cause::Modal | Cause::Switch | Cause::Close => {
            let (command, params) = match cause {
                Cause::Modal => ("help.about", json!({})),
                Cause::Switch => ("doc.switch", json!({"index":1})),
                _ => ("file.close", json!({"force":true})),
            };
            c.ui.app.run(command, params).unwrap();
            c.run(vec![]);
        }
        Cause::Focus => c.run(vec![Event::WindowFocused(false)]),
        Cause::Viewport => c.run_custom(vec![], &[], |raw| raw.screen_rect = Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(900.0, 1440.0)))),
        Cause::Allocation => {
            c.ui.app.canvas.rect.max.x -= 20.0;
            c.run(vec![]);
        }
        Cause::Unavailable => {
            c.unavailable(vec![]);
            c.run(vec![]);
        }
        Cause::Panic => {
            let completed = crate::windows::guard(&mut c.ui.app, &c.ui.ctx, |_| panic!("synthetic UI control panic"));
            c.native.finish_frame(&c.ui.app, &c.ui.ctx, completed);
            c.run(vec![]);
        }
        _ => c.run(vec![]),
    }
}
fn assert_pointer_ui(h: &Host, c: &Host, cause: Cause) {
    let a = &h.ui.app.ui.comments;
    let e = &c.ui.app.ui.comments;
    assert_eq!(
        (a.open, &a.draft, &a.reply, &a.editing, &a.search, a.filter, a.sort),
        (e.open, &e.draft, &e.reply, &e.editing, &e.search, e.filter, e.sort),
        "{cause:?}: comment UI"
    );
    assert_eq!(crate::proto_panel::editor(&h.ui.ctx), crate::proto_panel::editor(&c.ui.ctx), "{cause:?}: prototype editor");
    assert!(!a.dragging());
}
fn open_ui_matrix(prototype: bool) {
    let fixture = if prototype { Fixture::Prototype } else { Fixture::CommentPin };
    for cause in CAUSES {
        let (mut h, from, to) = seed(fixture, cause);
        if prototype {
            let source = h.ui.app.session.selection()[0];
            let dest = h.ui.app.session.doc().unwrap().nodes().find(|n| n.kind.is_frame_like()).unwrap().id;
            h.ui.app.run("proto.interaction.add", json!({"id":source,"dest":dest})).unwrap();
            crate::proto_panel::open_editor(&h.ui.ctx, Some((source, 0)));
        } else {
            let thread = h.ui.app.session.doc().unwrap().comments[0].id;
            h.ui.app.ui.comments.show(thread);
            h.ui.app.ui.comments.reply = "Keep this synthetic reply".into();
            h.ui.app.ui.comments.search = "Synthetic".into();
        }
        for _ in 0..3 {
            h.run(vec![]);
        }
        if prototype {
            assert!(crate::proto_panel::editor(&h.ui.ctx).is_some());
        } else {
            assert!(h.ui.app.ui.comments.open.is_some());
        }
        let mut c = ui_control(&h);
        let mut engine = control(&h);
        h.pointer = from;
        h.start();
        h.moved(to);
        assert!(h.ui.app.canvas_pointer.is_some(), "{fixture:?}/{cause:?}: enrolled preview with an existing open UI");
        if !prototype {
            // This independent panel filter is outside the pointer UI snapshot.
            h.ui.app.ui.comments.search.clear();
            c.ui.app.ui.comments.search.clear();
        }
        interrupt(&mut h, &mut engine, cause);
        ui_control_interrupt(&mut c, cause);
        assert_engine(&h.ui.app.session, &engine, fixture, cause);
        assert_pointer_ui(&h, &c, cause);
        match cause {
            Cause::Focus => h.run_checked(
                vec![touch(1, TouchPhase::End, h.pointer), button(h.pointer, false), Event::PointerGone],
                &[crate::touch_events::Issue::MissingStart, crate::touch_events::Issue::PhysicalInput],
            ),
            Cause::Cancel => h.run(vec![button(h.pointer, false)]),
            _ => h.end(),
        }
        if cause == Cause::Second {
            h.run(vec![touch(2, TouchPhase::End, to + vec2(60.0, 20.0)), Event::PointerGone]);
        }
        c.run(vec![]);
        assert_engine(&h.ui.app.session, &engine, fixture, cause);
        assert_pointer_ui(&h, &c, cause);
    }
}
#[test]
fn ipad_native_open_comment_thread_survives_interruption_and_late_release() {
    open_ui_matrix(false);
}
#[test]
fn ipad_native_open_prototype_editor_survives_interruption_and_late_release() {
    open_ui_matrix(true);
}
