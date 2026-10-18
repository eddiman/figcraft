use super::{is_copy_command, is_cut_command, is_paste_command};
use egui::{Key, Modifiers};

fn command() -> Modifiers {
    Modifiers {
        command: true,
        mac_cmd: true,
        ..Default::default()
    }
}

#[test]
fn ipad_host_plain_clipboard_chords_keep_their_intents() {
    assert!(is_copy_command(command(), Key::C));
    assert!(is_cut_command(command(), Key::X));
    assert!(is_paste_command(command(), Key::V));
    assert!(!is_copy_command(command(), Key::X));
    assert!(!is_cut_command(command(), Key::V));
    assert!(!is_paste_command(command(), Key::C));
}

#[test]
fn ipad_host_modified_clipboard_chords_reach_editor_bindings() {
    for modifiers in [
        Modifiers {
            shift: true,
            ..command()
        },
        Modifiers {
            alt: true,
            ..command()
        },
        Modifiers {
            ctrl: true,
            ..command()
        },
        Modifiers {
            shift: true,
            alt: true,
            ..command()
        },
    ] {
        assert!(!is_copy_command(modifiers, Key::C), "{modifiers:?}");
        assert!(!is_cut_command(modifiers, Key::X), "{modifiers:?}");
        assert!(!is_paste_command(modifiers, Key::V), "{modifiers:?}");
    }
}

#[test]
fn ipad_host_dedicated_clipboard_keys_keep_their_intents() {
    for modifiers in [
        Modifiers::NONE,
        command(),
        Modifiers {
            shift: true,
            alt: true,
            ctrl: true,
            ..command()
        },
    ] {
        assert!(is_copy_command(modifiers, Key::Copy));
        assert!(is_cut_command(modifiers, Key::Cut));
        assert!(is_paste_command(modifiers, Key::Paste));
    }
}

#[test]
fn ipad_host_plain_letters_do_not_become_clipboard_intents() {
    for key in [Key::C, Key::X, Key::V] {
        assert!(!is_copy_command(Modifiers::NONE, key));
        assert!(!is_cut_command(Modifiers::NONE, key));
        assert!(!is_paste_command(Modifiers::NONE, key));
    }
}
