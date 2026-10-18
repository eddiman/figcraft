use crate::{Error, Report};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, runtime::AnyObject};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString, NSUTF8StringEncoding, NSUUID};
use objc2_ui_kit::{UIApplication, UIApplicationState, UIPasteboard};

const LIMIT: usize = 8 * 1024 * 1024;
const NAME: &str = "org.figcraft.t13-owned-text-probe";
const TOKEN_TYPE: &str = "org.figcraft.t13.copy-token";
pub(crate) const GENERAL_HOST: &str = "org.figcraft.t13.general-host";

fn board_for(name: &str) -> Result<Retained<UIPasteboard>, Error> {
    if name == GENERAL_HOST {
        Ok(UIPasteboard::generalPasteboard())
    } else {
        UIPasteboard::pasteboardWithName_create(&NSString::from_str(name), false).ok_or(Error::Unavailable)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Content {
    Empty,
    Unsupported,
    Text(String),
}
struct Snapshot {
    count: isize,
    content: Content,
    token: Option<String>,
    image: Option<crate::snapshot::RgbaImage>,
}
// This guard is constructed and dropped inside the checked main-thread entry.
// It cannot escape, including during Rust unwinding.
struct Cleanup {
    name: Retained<NSString>,
    _main: MainThreadMarker,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        UIPasteboard::removePasteboardWithName(&self.name);
    }
}

fn snapshot(board: &UIPasteboard, _main: MainThreadMarker) -> Result<Snapshot, Error> {
    // SAFETY: The checked entry owns every access on the UIKit main thread.
    // No board reference escapes, and this function installs no callback.
    let before = unsafe { board.changeCount() };
    // SAFETY: Same main-thread serialization; the returned NSString is retained.
    let text = unsafe { board.string() };
    // SAFETY: Same main-thread serialization, without concurrent local access.
    let has_strings = unsafe { board.hasStrings() };
    // SAFETY: Same main-thread serialization. Negative counts fail below.
    let items = unsafe { board.numberOfItems() };
    let encoded =
        board.dataForPasteboardType(&NSString::from_str("public.png")).or_else(|| board.dataForPasteboardType(&NSString::from_str("public.jpeg")));
    let image = match encoded {
        Some(data) => {
            if data.len() > crate::image_data::ENCODED_LIMIT {
                return Err(Error::TooLarge);
            }
            Some(crate::image_data::decode(&data.to_vec()).map_err(|error| match error {
                crate::snapshot::SnapshotError::TooLarge => Error::TooLarge,
                _ => Error::InvalidImage,
            })?)
        }
        None => None,
    };
    let token = match board.dataForPasteboardType(&NSString::from_str(TOKEN_TYPE)) {
        Some(data) => {
            if data.len() != 36 {
                return Err(Error::InvalidToken);
            }
            let bytes = data.to_vec();
            let token = String::from_utf8(bytes).map_err(|_| Error::InvalidToken)?;
            if !token.bytes().enumerate().all(|(i, byte)| if matches!(i, 8 | 13 | 18 | 23) { byte == b'-' } else { byte.is_ascii_hexdigit() }) {
                return Err(Error::InvalidToken);
            }
            Some(token)
        }
        None => None,
    };
    let content = match text {
        Some(text) => {
            // Bound the allocation before converting the immutable retained value.
            // UTF-16 length bounds malformed/unsupported UTF-8 conversion as well.
            if text.length() > LIMIT || text.lengthOfBytesUsingEncoding(NSUTF8StringEncoding) > LIMIT {
                return Err(Error::TooLarge);
            }
            let text = text.to_string();
            if text.len() > LIMIT {
                return Err(Error::TooLarge);
            }
            Content::Text(text)
        }
        None if has_strings => return Err(Error::Unavailable),
        None if items == 0 => Content::Empty,
        None if image.is_some() => Content::Empty,
        None if items > 0 => Content::Unsupported,
        None => return Err(Error::Unavailable),
    };
    // SAFETY: Same main-thread serialization. Other processes can still update
    // pasteboard content, so compare the generation around this entire read.
    let after = unsafe { board.changeCount() };
    if before < 0 || after < 0 {
        return Err(Error::Unavailable);
    }
    if before != after {
        return Err(Error::Changed);
    }
    Ok(Snapshot { count: after, content, token, image })
}

fn write(board: &UIPasteboard, text: String, main: MainThreadMarker) -> Result<crate::WriteReceipt, Error> {
    if text.len() > LIMIT {
        return Err(Error::TooLarge);
    }
    let token = NSUUID::UUID().UUIDString().to_string();
    let text_key = NSString::from_str("public.utf8-plain-text");
    let token_key = NSString::from_str(TOKEN_TYPE);
    let text_data = NSData::with_bytes(text.as_bytes());
    let token_data = NSData::with_bytes(token.as_bytes());
    let dictionary = NSDictionary::<NSString, AnyObject>::from_slices(&[&*text_key, &*token_key], &[&*text_data, &*token_data]);
    let items = NSArray::from_retained_slice(&[dictionary]);
    // SAFETY: Both keys are NSString type identifiers and both values are immutable
    // NSData, as UIPasteboard requires for these data representations. All values
    // remain retained during this single synchronous main-thread operation. UIKit
    // copies the item; no native object or borrowed Rust buffer escapes.
    unsafe { board.setItems(&items) };
    let readback = snapshot(board, main)?;
    match readback.content {
        Content::Text(found) if found == text && readback.token.as_deref() == Some(&token) => {
            Ok(crate::WriteReceipt { change_count: readback.count, token })
        }
        _ => Err(Error::ReadbackMismatch),
    }
}

/// Runs only synthetic named-board cases. No system clipboard is read or written.
/// The result contains case labels and counts, without clipboard strings.
pub fn run_named_cases() -> Result<Report, Error> {
    let main = MainThreadMarker::new().ok_or(Error::WrongThread)?;
    if UIApplication::sharedApplication(main).applicationState() != UIApplicationState::Active {
        return Err(Error::Inactive);
    }
    std::panic::catch_unwind(|| {
        let mut report = run(main)?;
        if UIPasteboard::pasteboardWithName_create(&NSString::from_str(NAME), false).is_some() {
            return Err(Error::ReadbackMismatch);
        }
        report.passed.push("named board removed after run");
        Ok(report)
    })
    .unwrap_or(Err(Error::Panicked))
}

fn run(main: MainThreadMarker) -> Result<Report, Error> {
    let name = NSString::from_str(NAME);
    // Only the probe's fixed private name is removed; never the general board.
    UIPasteboard::removePasteboardWithName(&name);
    let _cleanup = Cleanup { name: name.clone(), _main: main };
    let board = UIPasteboard::pasteboardWithName_create(&name, true).ok_or(Error::Unavailable)?;
    let mut report = Report { passed: Vec::new(), change_counts: Vec::new() };
    if snapshot(&board, main)?.content != Content::Empty {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("new named board is empty");
    let first = write(&board, "equal".to_owned(), main)?;
    let second = write(&board, "equal".to_owned(), main)?;
    if first.token == second.token || snapshot(&board, main)?.token.as_deref() != Some(&second.token) {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("equal text writes have distinct atomic tokens");
    for (label, value) in [
        ("ASCII roundtrip", "synthetic clipboard"),
        ("Unicode roundtrip", "Å雪🦀"),
        ("repeated equal write one", "same"),
        ("repeated equal write two", "same"),
        ("empty string roundtrip", ""),
    ] {
        report.change_counts.push(write(&board, value.to_owned(), main)?.change_count);
        report.passed.push(label);
    }
    let limit = "a".repeat(LIMIT);
    report.change_counts.push(write(&board, limit, main)?.change_count);
    report.passed.push("8 MiB write roundtrip");
    let before = snapshot(&board, main)?;
    if write(&board, "é".repeat(LIMIT / 2 + 1), main) != Err(Error::TooLarge) {
        return Err(Error::ReadbackMismatch);
    }
    let after = snapshot(&board, main)?;
    if before.count != after.count || before.content != after.content {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("oversized UTF-8 preserves named board");
    let oversized_native = NSString::from_str(&"é".repeat(LIMIT / 2 + 1));
    // SAFETY: This synthetic value and board stay on the checked main thread.
    // This bypasses the writer only to verify the native reader's byte bound.
    unsafe { board.setString(Some(&oversized_native)) };
    if !matches!(snapshot(&board, main), Err(Error::TooLarge)) {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("oversized native string read rejected");
    let text = NSData::with_bytes(b"external synthetic text");
    let key = NSString::from_str("public.utf8-plain-text");
    let item = NSDictionary::<NSString, AnyObject>::from_slices(&[&*key], &[&*text]);
    // SAFETY: Correct NSString/NSData representations, retained locally on the
    // checked main thread, exactly as for the reviewed atomic writer.
    unsafe { board.setItems(&NSArray::from_retained_slice(&[item])) };
    let external = snapshot(&board, main)?;
    if external.token.is_some() || external.content != Content::Text("external synthetic text".to_owned()) {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("external text has no internal ownership token");
    let key = NSString::from_str("org.figcraft.t13.unsupported");
    let item = NSDictionary::<NSString, AnyObject>::from_slices(&[&*key], &[&*text]);
    // SAFETY: A private type with NSData is a valid pasteboard representation.
    unsafe { board.setItems(&NSArray::from_retained_slice(&[item])) };
    if snapshot(&board, main)?.content != Content::Unsupported {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("unsupported named-board content is explicit");
    let key = NSString::from_str(TOKEN_TYPE);
    let item = NSDictionary::<NSString, AnyObject>::from_slices(&[&*key], &[&*text]);
    // SAFETY: NSData is valid for this private type. Its invalid bytes exercise
    // the bounded reader rather than violating an Objective-C type contract.
    unsafe { board.setItems(&NSArray::from_retained_slice(&[item])) };
    if !matches!(snapshot(&board, main), Err(Error::InvalidToken)) {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("malformed token fails before ownership inference");
    if !matches!(std::thread::spawn(run_named_cases).join(), Ok(Err(Error::WrongThread))) {
        return Err(Error::ReadbackMismatch);
    }
    report.passed.push("worker entry rejected before UIKit access");
    // The guard removes the board after its last retained object drops.
    Ok(report)
}

fn checked_main() -> Result<MainThreadMarker, Error> {
    let main = MainThreadMarker::new().ok_or(Error::WrongThread)?;
    if UIApplication::sharedApplication(main).applicationState() != UIApplicationState::Active {
        return Err(Error::Inactive);
    }
    Ok(main)
}

pub(crate) fn open_named(name: &str) -> Result<(), Error> {
    std::panic::catch_unwind(|| {
        let _main = checked_main()?;
        let name = NSString::from_str(name);
        UIPasteboard::pasteboardWithName_create(&name, true).ok_or(Error::Unavailable)?;
        Ok(())
    })
    .unwrap_or(Err(Error::Panicked))
}

pub(crate) fn write_named(name: &str, text: String) -> Result<crate::WriteReceipt, Error> {
    std::panic::catch_unwind(|| {
        let main = checked_main()?;
        // Do not recreate a board after its owner has removed it.
        let board = board_for(name)?;
        write(&board, text, main)
    })
    .unwrap_or(Err(Error::Panicked))
}

pub(crate) fn read_named(name: &str) -> Result<crate::TextSnapshot, Error> {
    std::panic::catch_unwind(|| {
        let main = checked_main()?;
        let board = board_for(name)?;
        let read = snapshot(&board, main)?;
        let (text, unsupported) = match read.content {
            Content::Empty => (None, false),
            Content::Unsupported => (None, true),
            Content::Text(text) => (Some(text), false),
        };
        Ok(crate::TextSnapshot { change_count: read.count, text, unsupported, token: read.token, image: read.image })
    })
    .unwrap_or(Err(Error::Panicked))
}

pub(crate) fn remove_named(name: &str) {
    if name == GENERAL_HOST {
        return;
    }
    // NamedHost is not Send/Sync, and is created on the checked main thread.
    // The additional runtime guard prevents UIKit access if this invariant changes.
    if MainThreadMarker::new().is_some() {
        UIPasteboard::removePasteboardWithName(&NSString::from_str(name));
    }
}

pub(crate) fn write_image_named(name: &str, image: crate::snapshot::RgbaImage) -> Result<crate::WriteReceipt, Error> {
    std::panic::catch_unwind(|| {
        let main = checked_main()?;
        let encoded = crate::image_data::encode(&image).map_err(|error| match error {
            crate::snapshot::SnapshotError::TooLarge => Error::TooLarge,
            _ => Error::InvalidImage,
        })?;
        let board = board_for(name)?;
        let token = NSUUID::UUID().UUIDString().to_string();
        let key = NSString::from_str("public.png");
        let token_key = NSString::from_str(TOKEN_TYPE);
        let data = NSData::with_bytes(&encoded);
        let token_data = NSData::with_bytes(token.as_bytes());
        let item = NSDictionary::<NSString, AnyObject>::from_slices(&[&*key, &*token_key], &[&*data, &*token_data]);
        // SAFETY: Immutable NSData representations and NSString keys are retained
        // for this synchronous main-thread write, as for the reviewed text writer.
        unsafe { board.setItems(&NSArray::from_retained_slice(&[item])) };
        let read = snapshot(&board, main)?;
        if read.image.as_ref() != Some(&image) || read.token.as_deref() != Some(&token) {
            return Err(Error::ReadbackMismatch);
        }
        Ok(crate::WriteReceipt { change_count: read.count, token })
    })
    .unwrap_or(Err(Error::Panicked))
}
