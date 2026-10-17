use super::*;

fn pending(styles: &[&str]) -> FontDb {
    let mut db = FontDb::default();
    let entries = styles.iter().map(|s| Pending { path: format!("no-such-dir/{s}.ttf").into(), index: 0, style: (*s).into() }).collect();
    db.pending.insert("cjk ui".into(), ("CJK UI".into(), entries));
    db
}

fn left(db: &FontDb) -> Vec<String> {
    db.pending.get("cjk ui").map(|(_, v)| v.iter().map(|e| e.style.clone()).collect()).unwrap_or_default()
}

#[test]
fn find_one_reads_only_the_nearest_style() {
    // The files don't exist: what matters is which entry is tried, and that the rest stay pending.
    let mut db = pending(&["Bold", "Light", "Regular", "Italic"]);
    assert!(db.find_one("CJK UI", "Regular").is_none());
    assert_eq!(left(&db), ["Bold", "Light", "Italic"]);
    assert!(db.has_family("cjk ui"), "the family is still offered");
    // No exact name: the nearest weight, upright before italic.
    let mut db = pending(&["W6", "Italic", "W3", "Bold"]);
    db.find_one("CJK UI", "Regular");
    assert_eq!(left(&db), ["W6", "Italic", "Bold"]);
    // `find` reads the whole family.
    db.find("CJK UI", "Regular");
    assert!(left(&db).is_empty() && !db.has_family("cjk ui"));
    assert_eq!(pending_weight("W3"), 300);
    assert_eq!(pending_weight("SemiBold"), 600);
    assert_eq!(pending_weight("W12"), 400);
}

#[test]
fn system_fonts_are_scanned_once() {
    // Racing callers (the desktop's background scan, on-demand lookups, tests) share one walk.
    let threads: Vec<_> = (0..4).map(|_| std::thread::spawn(scan_system_fonts)).collect();
    for t in threads {
        t.join().unwrap();
    }
    scan_system_fonts_on_demand();
    let _ = db();
    scan_system_fonts();
    let walked = !cfg!(any(target_os = "ios", target_arch = "wasm32")) && !system_fonts_opted_out();
    assert_eq!(SCANS.load(Ordering::Relaxed), usize::from(walked));
    assert_eq!(system_fonts_scanned(), walked);
}

#[test]
fn the_scan_reads_names_like_a_whole_file_read() {
    // Every installed font file (none on CI): the partial read finds the same faces and names.
    let mut files = Vec::new();
    for dir in system_font_dirs() {
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| ["ttf", "otf", "ttc", "otc"].contains(&x.to_ascii_lowercase().to_str().unwrap_or(""))) {
                    files.push(p);
                }
            }
        }
    }
    for p in files {
        let Ok(whole) = std::fs::read(&p) else { continue };
        assert_eq!(faces_in(&index_bytes(&p, whole.len()).unwrap()), faces_in(&whole), "{}", p.display());
    }
}

#[test]
fn ipad_embedded_font_available() {
    if CRAFT_FONTS.is_empty() && std::env::var_os("FIGCRAFT_IPAD_FONT_TESTS_REQUIRED").is_none() {
        eprintln!("iPad font coverage not exercised: optional craft-fonts input absent");
        return;
    }
    let samples = [
        ("BIZ UDPGothic", "Regular", "FigCraft Åå Ææ Øø é ñ あ漢"),
        ("BIZ UDPGothic", "Bold", "FigCraft Åå Ææ Øø é ñ"),
        ("Shippori Mincho", "Regular", "日本語"),
        ("BIZ UDMincho", "Regular", "日本語"),
        ("Noto Sans CJK SC", "Regular", "们漢한"),
        ("Noto Sans Arabic", "Regular", "مرحبا"),
        ("Noto Sans Thai", "Regular", "ไทย"),
    ];
    for (family, style, text) in samples {
        let input = CRAFT_FONTS.iter().find(|f| f.family == family && f.style == style).unwrap();
        let face = Face::from_bytes(FontBytes::Static(input.bytes), 0, Some(family), Some(style)).unwrap();
        assert!(text.chars().all(|c| face.has_char(c)), "{family}/{style}: {text}");
    }
}

#[test]
fn ipad_embedded_document_scripts_and_missing_font_fallback() {
    if CRAFT_FONTS.is_empty() && std::env::var_os("FIGCRAFT_IPAD_FONT_TESTS_REQUIRED").is_none() {
        eprintln!("iPad document font coverage not exercised: optional input absent");
        return;
    }
    let mut isolated = FontDb::default();
    for input in CRAFT_FONTS {
        isolated.add(Face::from_bytes(FontBytes::Static(input.bytes), 0, Some(input.family), Some(input.style)).unwrap());
    }
    assert!(isolated.find("Synthetic unavailable font", "Regular").is_none());
    let fallback = isolated.default_face_for(true).unwrap();
    assert_eq!(fallback.family, "BIZ UDPGothic");
    for sample in ["FigCraft Åæø", "日本語", "们漢한", "مرحبا", "ไทย"] {
        let mut text = figcraft_doc::TextData::new(sample);
        text.style.font_family = fallback.family.clone();
        let before = text.clone();
        let layout = crate::layout(&text, None, None);
        assert!(!layout.glyphs.is_empty(), "{sample}: document glyphs");
        assert!(layout.glyphs.iter().all(|g| g.gid != 0 && g.advance.is_finite()), "{sample}: no missing glyphs");
        assert!(layout.width.is_finite() && layout.width > 0.0);
        assert_eq!(text, before, "font resolution cannot rewrite requested document styles");
    }
}

#[test]
fn ipad_no_unix_font_scan() {
    if cfg!(any(target_os = "ios", target_arch = "wasm32")) {
        let mut isolated = FontDb::default();
        isolated.scan_system();
        assert!(!isolated.system_scanned);
        assert!(isolated.pending.is_empty());
        assert!(system_font_dirs().is_empty());
    } else {
        // The iOS branch is compiled by IOS-CHECK; this host cannot execute it.
        assert!(!system_font_dirs().is_empty());
    }
}
