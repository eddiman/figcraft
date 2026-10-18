//! Process-local acknowledgement of the exact engine clipboard mirrored natively.
use figcraft_engine::Clipboard;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct Association {
    receipt: egui::PasteIdentity,
    clipboard: Clipboard,
}

impl Association {
    pub(crate) fn new(receipt: egui::PasteIdentity, clipboard: &Clipboard) -> Self {
        Self { receipt, clipboard: clipboard.clone() }
    }

    pub(crate) fn matches(&self, identity: Option<&egui::PasteIdentity>, current: &Clipboard) -> bool {
        let source_matches = match (&self.clipboard.source, &current.source) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        identity == Some(&self.receipt)
            && self.clipboard.same_copy_operation(current)
            && source_matches
            && self.clipboard.nodes == current.nodes
            && self.clipboard.images == current.images
            && self.clipboard.props == current.props
            && self.clipboard.bounds == current.bounds
            && self.clipboard.source_doc == current.source_doc
            && self.clipboard.source_path == current.source_path
    }
}

#[cfg(test)]
mod tests {
    use crate::{FigApp, Services, menus};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Board {
        text: String,
        identity: Option<egui::PasteIdentity>,
        sequence: u64,
        fail: bool,
    }
    struct Adapter(Arc<Mutex<Board>>);
    impl Services for Adapter {
        fn clipboard_requires_confirmation(&self) -> bool {
            true
        }
        fn clipboard_try_set_layers(&mut self, text: &str) -> Result<Option<egui::PasteIdentity>, String> {
            let mut board = self.0.lock().unwrap();
            if board.fail {
                return Err("synthetic denial".into());
            }
            board.sequence += 1;
            let identity = egui::PasteIdentity { token: format!("00000000-0000-0000-0000-{:012X}", board.sequence), generation: board.sequence };
            board.text = text.into();
            board.identity = Some(identity.clone());
            Ok(Some(identity))
        }
    }
    fn setup() -> (FigApp, Arc<Mutex<Board>>) {
        let board = Arc::new(Mutex::new(Board::default()));
        let mut app = FigApp::new(figcraft_engine::Session::new(), Box::new(Adapter(board.clone())));
        app.run("shape.rectangle", json!({"x":0,"y":0,"width":10,"height":10}));
        (app, board)
    }
    fn snapshot(board: &Arc<Mutex<Board>>) -> egui::PasteOutcome {
        let board = board.lock().unwrap();
        egui::PasteOutcome::Content(egui::PasteContent { text: Some(board.text.clone()), image: None, identity: board.identity.clone() })
    }
    #[test]
    fn ipad_layer_receipt_pastes_exact_internal_copy_and_cut() {
        for command in ["edit.copy", "edit.cut"] {
            let (mut app, board) = setup();
            menus::invoke(&mut app, command);
            assert!(app.ui.native_copy.is_some());
            let outcome = snapshot(&board);
            let expected = app.session.clipboard.clone();
            let before = app.session.doc().unwrap().nodes().count();
            menus::paste_owned(&mut app, &outcome);
            let pasted = app.session.selection();
            let node = app.session.doc().unwrap().node(pasted[0]).unwrap();
            assert_eq!(node.kind, expected.nodes[0].1[0].kind);
            assert_eq!(node.name, expected.nodes[0].1[0].name);
            assert_eq!(app.session.doc().unwrap().nodes().count(), before + 1);
            // Engine Paste names its history entry; importing SVG follows a different path.
            assert_eq!(app.session.ds().unwrap().history.undo.last().unwrap().label, "Paste");
        }
    }
    #[test]
    fn ipad_layer_receipt_recopy_mutation_and_external_identity_invalidate() {
        for cause in 0..4 {
            let (mut app, board) = setup();
            menus::invoke(&mut app, "edit.copy");
            let mut outcome = snapshot(&board);
            match cause {
                0 => {
                    app.run("edit.copy", json!({}));
                }
                1 => {
                    app.session.clipboard.nodes.clear();
                }
                2 => {
                    if let egui::PasteOutcome::Content(c) = &mut outcome {
                        c.identity.as_mut().unwrap().generation += 1;
                    }
                }
                _ => {
                    if let egui::PasteOutcome::Content(c) = &mut outcome {
                        c.identity.as_mut().unwrap().token = "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF".into();
                    }
                }
            }
            let identity = match &outcome {
                egui::PasteOutcome::Content(content) => content.identity.as_ref(),
                _ => None,
            };
            assert!(!app.ui.native_copy.as_ref().unwrap().matches(identity, &app.session.clipboard));
            menus::paste_owned(&mut app, &outcome);
            assert_ne!(app.session.ds().unwrap().history.undo.last().unwrap().label, "Paste");
        }
    }
    #[test]
    fn ipad_layer_denied_cut_preserves_document_and_invalidates_association() {
        let (mut app, board) = setup();
        menus::invoke(&mut app, "edit.copy");
        let before = app.session.ds().unwrap().clone();
        let clipboard = app.session.clipboard.clone();
        board.lock().unwrap().fail = true;
        menus::invoke(&mut app, "edit.cut");
        let after = app.session.ds().unwrap();
        assert_eq!(after.doc, before.doc);
        assert_eq!(after.selection, before.selection);
        assert_eq!(app.session.undo_depth(), (before.history.undo.len(), before.history.redo.len()));
        assert!(clipboard.same_copy_operation(&app.session.clipboard));
        assert!(app.ui.native_copy.is_none());
    }
    #[test]
    fn ipad_layer_engine_failure_after_write_cannot_retain_association() {
        let (mut app, board) = setup();
        menus::invoke(&mut app, "edit.copy");
        // A stale selection reaches SVG fallback and a confirmed write, then fails target validation.
        app.session.ds_mut().unwrap().selection = vec![figcraft_doc::NodeId(u64::MAX)];
        let before = board.lock().unwrap().sequence;
        menus::invoke(&mut app, "edit.copy");
        assert_eq!(board.lock().unwrap().sequence, before + 1);
        assert!(app.ui.native_copy.is_none());
    }
}
