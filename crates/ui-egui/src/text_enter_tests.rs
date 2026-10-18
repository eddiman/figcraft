//! Enter and Escape on text, and the page a new file gets in each theme, driven through egui
//! headless: Enter on a selected text layer edits it with all of its text selected (what a
//! double-click into it does), typing replaces it, and Escape leaves editing with the layer still
//! selected; File ▸ New in the dark theme makes a dark page.

use egui::{Event, Key, Modifiers};
use figcraft_color::Color;
use figcraft_doc::{NodeId, NodeKind};
use figcraft_engine::Session;
use serde_json::json;

use crate::layers_drag_tests::Ui;

fn press(u: &mut Ui, key: Key) {
    let modifiers = Modifiers::NONE;
    u.frame(vec![
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers },
        Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers },
    ]);
}

fn text(u: &Ui, id: NodeId) -> String {
    match &u.app.session.doc().unwrap().node(id).unwrap().kind {
        NodeKind::Text(t) => t.characters.clone(),
        _ => panic!("not text"),
    }
}

fn page_colour(u: &Ui) -> Color {
    let ds = u.app.session.ds().unwrap();
    match ds.doc.node(ds.page).unwrap().kind {
        NodeKind::Page { background, .. } => background,
        _ => panic!("not a page"),
    }
}

#[test]
fn enter_edits_a_selected_text_with_all_of_it_selected_and_escape_keeps_the_layer_selected() {
    let mut s = Session::new();
    let t = NodeId(s.execute("text.create", &json!({"text": "Hello world", "x": 20, "y": 20})).unwrap()["id"].as_u64().unwrap());
    s.execute("select.set", &json!({"ids": [t]})).unwrap();
    let mut u = Ui::new(s);
    press(&mut u, Key::Enter);
    let te = u.app.session.ds().unwrap().text_edit.clone().expect("Enter starts editing");
    assert_eq!((te.id, te.anchor.min(te.caret), te.anchor.max(te.caret)), (t, 0, 11), "all of the text is selected");
    // Typing replaces the selection, as after a double-click into the text.
    u.frame(vec![Event::Text("Bye".into())]);
    assert_eq!(text(&u, t), "Bye");
    press(&mut u, Key::Escape);
    let ds = u.app.session.ds().unwrap();
    assert!(ds.text_edit.is_none(), "Escape leaves editing");
    assert_eq!(ds.selection, vec![t], "the layer stays selected");
    assert_eq!(text(&u, t), "Bye", "what was typed is kept");
    // One undo step for the whole edit.
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(text(&u, t), "Hello world");
    // A second Escape deselects, as it does for any layer.
    press(&mut u, Key::Escape);
    assert!(u.app.session.selection().is_empty());
}

#[test]
fn a_new_file_takes_the_page_colour_of_the_theme_and_keeps_it() {
    let mut s = Session::new();
    s.prefs.theme = "dark".into();
    let mut u = Ui::new(s);
    assert_eq!(page_colour(&u), Color::DARK_PAGE, "the file the app opened with, untouched");
    u.app.run("file.new", json!({}));
    assert_eq!(page_colour(&u), Color::DARK_PAGE);
    u.app.run("view.themeLight", json!({}));
    u.frame(vec![]);
    assert_eq!(page_colour(&u), Color::DARK_PAGE, "a change of theme leaves the page alone");
    u.app.run("file.new", json!({}));
    assert_eq!(page_colour(&u), Color::DEFAULT_PAGE, "a file made in the light theme");
    u.app.run("view.themeDark", json!({}));
    u.frame(vec![]);
    u.app.run("page.add", json!({}));
    assert_eq!(page_colour(&u), Color::DEFAULT_PAGE, "a page added in the dark theme follows its light page");
    u.app.run("file.new", json!({}));
    assert_eq!(page_colour(&u), Color::DARK_PAGE, "a file made in the dark theme");
}

#[test]
fn ipad_text_unicode_replacement_paste_and_single_undo() {
    let mut session = Session::new();
    let id = NodeId(session.execute("text.create", &json!({"text": "Before", "x": 20, "y": 20})).unwrap()["id"].as_u64().unwrap());
    session.execute("select.set", &json!({"ids": [id]})).unwrap();
    let mut u = Ui::new(session);
    let undo = u.app.session.undo_depth().0;
    press(&mut u, Key::Enter);
    u.frame(vec![Event::Text("A\u{1f469}\u{200d}\u{1f4bb}B".into())]);
    assert_eq!(text(&u, id), "A\u{1f469}\u{200d}\u{1f4bb}B");
    figcraft_engine::textedit::set_caret(&mut u.app.session, 1, false).unwrap();
    figcraft_engine::textedit::set_caret(&mut u.app.session, 4, true).unwrap();
    u.frame(vec![Event::Text("\u{e9}".into())]);
    assert_eq!(text(&u, id), "A\u{e9}B", "replace the whole multi-scalar emoji selection");
    u.frame(vec![Event::OwnedPaste(egui::PasteOutcome::Content(egui::PasteContent {
        text: Some("\r\n\u{65e5}\u{672c}\u{8a9e} \u{1f600}\rCafe\u{301}".into()),
        image: None,
        identity: None,
    }))]);
    assert_eq!(text(&u, id), "A\u{e9}\n\u{65e5}\u{672c}\u{8a9e} \u{1f600}\nCafe\u{301}B");
    press(&mut u, Key::Escape);
    assert_eq!(u.app.session.undo_depth().0, undo + 1);
    u.app.session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(text(&u, id), "Before");
}

#[test]
fn ipad_text_preedit_selection_and_single_commit() {
    for replace_selection in [false, true] {
        let mut session = Session::new();
        let id = NodeId(session.execute("text.create", &json!({"text": "Before", "x": 20, "y": 20})).unwrap()["id"].as_u64().unwrap());
        session.execute("select.set", &json!({"ids": [id]})).unwrap();
        let mut u = Ui::new(session);
        press(&mut u, Key::Enter);
        if !replace_selection {
            figcraft_engine::textedit::set_caret(&mut u.app.session, 6, false).unwrap();
        }
        let prefix = if replace_selection { "" } else { "Before" };
        for candidate in ["\u{306b}", "\u{65e5}\u{672c}"] {
            u.frame(vec![Event::Ime(egui::ImeEvent::Preedit { text: candidate.into(), active_range_chars: Some(0..candidate.chars().count()) })]);
            assert_eq!(text(&u, id), prefix, "composition removes the selection, but does not insert marked text");
            assert_eq!(u.app.session.ds().unwrap().text_edit.as_ref().unwrap().preedit.as_ref().unwrap().text, candidate);
        }
        u.frame(vec![Event::Ime(egui::ImeEvent::Commit("\u{65e5}\u{672c}".into()))]);
        let committed = format!("{prefix}\u{65e5}\u{672c}");
        assert_eq!(text(&u, id), committed);
        let edit = u.app.session.ds().unwrap().text_edit.as_ref().unwrap();
        assert!(edit.preedit.is_none());
        let caret = prefix.chars().count() + 2;
        assert_eq!((edit.anchor, edit.caret), (caret, caret));
        u.frame(vec![]);
        assert_eq!(text(&u, id), committed, "later frames cannot replay the commit");
        press(&mut u, Key::Escape);
        u.app.session.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(text(&u, id), "Before");
    }
}
