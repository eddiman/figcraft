//! D12's pinned external inputs. Font bytes stay outside this repository.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde_json::Value;

type Result<T> = std::result::Result<T, String>;

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| format!("missing font profile field: {key}"))
}

fn relative(value: &str) -> Result<&Path> {
    let path = Path::new(value);
    if path.components().any(|c| !matches!(c, Component::Normal(_))) || path.as_os_str().is_empty() {
        return Err(format!("invalid font profile path: {value}"));
    }
    Ok(path)
}

fn digest(path: &Path) -> Result<String> {
    let metadata = path.metadata().map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err(format!("font input is not a bounded regular file: {}", path.display()));
    }
    let output = Command::new("shasum").args(["-a", "256"]).arg(path).output().map_err(|e| format!("shasum: {e}"))?;
    if !output.status.success() {
        return Err(format!("shasum failed: {}", path.display()));
    }
    String::from_utf8(output.stdout)
        .map_err(|e| e.to_string())?
        .split_whitespace()
        .next()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned)
        .ok_or_else(|| "shasum returned no SHA-256".into())
}

fn checked_file(dir: &Path, name: &str, expected: &str) -> Result<PathBuf> {
    let path = dir.join(relative(name)?).canonicalize().map_err(|e| format!("{name}: {e}"))?;
    if !path.starts_with(dir) || digest(&path)? != expected {
        return Err(format!("font input path or SHA-256 mismatch: {name}"));
    }
    Ok(path)
}

fn validate(dir: &Path, profile: &Value) -> Result<BTreeSet<String>> {
    let output = Command::new("git").arg("-C").arg(dir).args(["rev-parse", "HEAD"]).output().map_err(|e| e.to_string())?;
    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != field(profile, "commit")? {
        return Err("craft-fonts revision does not match maintenance/ipad-fonts.json".into());
    }
    checked_file(dir, field(profile, "manifest")?, field(profile, "manifest_sha256")?)?;
    checked_file(dir, "ATTRIBUTION.md", field(profile, "attribution_sha256")?)?;
    let fonts = profile.get("fonts").and_then(Value::as_array).filter(|f| !f.is_empty() && f.len() <= 64).ok_or("invalid font list")?;
    let mut licenses = BTreeSet::new();
    for font in fonts {
        if field(font, "license")? != "OFL-1.1" {
            return Err("unreviewed iPad font license".into());
        }
        checked_file(dir, field(font, "path")?, field(font, "sha256")?)?;
        let license = field(font, "license_path")?;
        checked_file(dir, license, field(font, "license_sha256")?)?;
        licenses.insert(license.to_owned());
    }
    Ok(licenses)
}

fn execute(root: &Path, args: &[String]) -> Result<()> {
    let mode = args.first().map(String::as_str).ok_or("usage: ipad-fonts check DIR | bundle DIR APP")?;
    if !matches!((mode, args.len()), ("check", 2) | ("bundle", 3)) {
        return Err("usage: ipad-fonts check DIR | bundle DIR APP".into());
    }
    let dir = Path::new(args.get(1).ok_or("missing craft-fonts directory")?).canonicalize().map_err(|e| e.to_string())?;
    let profile_path = root.join("maintenance/ipad-fonts.json");
    let bytes = std::fs::read(&profile_path).map_err(|e| e.to_string())?;
    let profile: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let licenses = validate(&dir, &profile)?;
    if mode == "bundle" {
        let app = Path::new(args.get(2).ok_or("missing app directory")?);
        if !app.is_dir() {
            return Err("app bundle directory is missing".into());
        }
        for name in licenses.into_iter().chain(["ATTRIBUTION.md".into()]) {
            let destination = app.join("FontLicenses").join(relative(&name)?);
            let parent = destination.parent().ok_or("license destination has no parent")?;
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            let expected = if name == "ATTRIBUTION.md" {
                field(&profile, "attribution_sha256")?
            } else {
                let font = profile["fonts"]
                    .as_array()
                    .and_then(|fonts| fonts.iter().find(|font| font["license_path"] == name))
                    .ok_or("copied license is absent from profile")?;
                field(font, "license_sha256")?
            };
            let source = dir.join(&name);
            std::fs::copy(&source, &destination).map_err(|e| e.to_string())?;
            if digest(&destination)? != expected {
                return Err(format!("copied font license differs: {name}"));
            }
        }
        std::fs::write(app.join("ipad-fonts.json"), bytes).map_err(|e| e.to_string())?;
    }
    eprintln!("iPad font profile: verified seven pinned font files and their licenses");
    Ok(())
}

pub fn run(root: &Path, args: &[String]) -> bool {
    match execute(root, args) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("iPad font inputs: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipad_font_profile_rejects_paths_outside_checkout() {
        for name in ["", "/tmp/font.ttf", "../font.ttf", "fonts/../../font.ttf", "./font.ttf"] {
            assert!(relative(name).is_err(), "{name}");
        }
        assert!(relative("fonts/noto-sans-thai/OFL.txt").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn ipad_font_profile_rejects_changed_missing_and_unlicensed_inputs() {
        let dir = crate::root().join("target").join(format!("ipad-font-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let revision = Command::new("git").arg("-C").arg(&dir).args(["rev-parse", "HEAD"]).output().unwrap();
        for name in ["manifest", "font", "license", "ATTRIBUTION.md"] {
            std::fs::write(dir.join(name), name).unwrap();
        }
        let mut profile = serde_json::json!({
            "commit":String::from_utf8_lossy(&revision.stdout).trim(),
            "manifest":"manifest", "manifest_sha256":digest(&dir.join("manifest")).unwrap(),
            "attribution_sha256":digest(&dir.join("ATTRIBUTION.md")).unwrap(),
            "fonts":[{"path":"font", "sha256":digest(&dir.join("font")).unwrap(),
                "license":"OFL-1.1", "license_path":"license", "license_sha256":digest(&dir.join("license")).unwrap()}]
        });
        assert!(validate(&dir, &profile).is_ok());
        std::fs::write(dir.join("font"), "changed bytes").unwrap();
        assert!(validate(&dir, &profile).unwrap_err().contains("SHA-256 mismatch"));
        std::fs::write(dir.join("font"), "font").unwrap();
        std::fs::remove_file(dir.join("license")).unwrap();
        assert!(validate(&dir, &profile).is_err());
        std::fs::write(dir.join("license"), "license").unwrap();
        profile["fonts"][0]["license"] = "unreviewed".into();
        assert!(validate(&dir, &profile).unwrap_err().contains("unreviewed"));
        profile["commit"] = "different revision".into();
        assert!(validate(&dir, &profile).unwrap_err().contains("revision"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
