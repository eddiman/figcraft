//! One archive member builder shared by encoding and persisted-content comparison.
use std::collections::BTreeSet;
use std::io::Cursor;
use std::sync::Arc;

use figcraft_doc::Document;
use serde::Serialize;
use serde_json::{Value, json};

use crate::{BRANCH_INDEX, Branch, FormatError, Snapshot, Version, io_err, is_valid_version_id, json_options, put};

#[derive(Clone, Debug)]
enum Data {
    Json { bytes: Arc<Vec<u8>>, value: Value },
    Binary(Arc<Vec<u8>>),
}
#[derive(Clone, Debug)]
struct Member {
    name: String,
    data: Data,
}
/// Immutable logical members of a native file. ZIP metadata and derived thumbnails
/// do not participate in [`Self::same_persisted_content`].
#[derive(Clone, Debug)]
pub struct ArchiveContent {
    members: Vec<Member>,
}
impl ArchiveContent {
    pub fn capture(doc: &Document, thumbnail: Option<&[u8]>, versions: &[Version], branches: &[Branch]) -> Result<Self, FormatError> {
        let mut content = Self { members: Vec::new() };
        content.document("document.json", doc)?;
        let mut written = BTreeSet::new();
        let branch_images = branches.iter().flat_map(|b| [&b.base.images, &b.head.images].into_iter().chain(b.versions.iter().map(Version::images)));
        for (hash, img) in std::iter::once(&doc.images).chain(versions.iter().map(Version::images)).chain(branch_images).flatten() {
            if written.insert(hash) {
                content.binary(&format!("images/{hash}"), img.bytes.clone());
            }
        }
        if let Some(bytes) = thumbnail {
            content.binary("thumbnail.png", Arc::new(bytes.to_vec()));
        }
        content.versions("versions", versions)?;
        if !branches.is_empty() {
            let mut ids = BTreeSet::new();
            let mut index = Vec::with_capacity(branches.len());
            for b in branches {
                if !is_valid_version_id(&b.id) || !ids.insert(b.id.as_str()) {
                    return Err(FormatError::Io(format!("invalid or duplicate branch id `{}`", b.id)));
                }
                content.document(&format!("branches/{}/base.json", b.id), &b.base)?;
                content.document(&format!("branches/{}/head.json", b.id), &b.head)?;
                content.versions(&format!("branches/{}/versions", b.id), &b.versions)?;
                let images = |d: &Document| d.images.keys().cloned().collect::<Vec<String>>();
                index.push(json!({"id": b.id, "name": b.name, "time": b.time, "client": b.client, "state": b.state,
                    "baseImages": images(&b.base), "headImages": images(&b.head)}));
            }
            content.json(BRANCH_INDEX, &json!({"branches": index}))?;
        }
        if let Some(lib) = &doc.published {
            crate::library::members(&mut content, lib, &written)?;
        }
        Ok(content)
    }
    pub(crate) fn document(&mut self, name: &str, doc: &Document) -> Result<(), FormatError> {
        let mut bytes = Vec::new();
        crate::write_json(doc, &mut bytes)?;
        self.raw_json(name, Arc::new(bytes))
    }
    pub(crate) fn json(&mut self, name: &str, value: &impl Serialize) -> Result<(), FormatError> {
        self.raw_json(name, Arc::new(serde_json::to_vec(value).map_err(io_err)?))
    }
    fn raw_json(&mut self, name: &str, bytes: Arc<Vec<u8>>) -> Result<(), FormatError> {
        let value = serde_json::from_slice(&bytes).map_err(|e| FormatError::Corrupt(format!("{name}: {e}")))?;
        self.members.push(Member { name: name.into(), data: Data::Json { bytes, value } });
        Ok(())
    }
    pub(crate) fn binary(&mut self, name: &str, bytes: Arc<Vec<u8>>) {
        self.members.push(Member { name: name.into(), data: Data::Binary(bytes) });
    }
    fn versions(&mut self, dir: &str, versions: &[Version]) -> Result<(), FormatError> {
        if versions.is_empty() {
            return Ok(());
        }
        let mut ids = BTreeSet::new();
        let mut index = Vec::with_capacity(versions.len());
        for v in versions {
            if !is_valid_version_id(&v.id) || !ids.insert(v.id.as_str()) {
                return Err(FormatError::Io(format!("invalid or duplicate version id `{}`", v.id)));
            }
            let name = format!("{dir}/{}.json", v.id);
            match &v.snapshot {
                Snapshot::Doc(d) => self.document(&name, d)?,
                Snapshot::Stored { json, .. } => self.raw_json(&name, json.clone())?,
            }
            let images: Vec<&String> = v.images().keys().collect();
            let mut entry = json!({"id": v.id, "name": v.name, "description": v.description, "time": v.time, "images": images});
            if v.auto {
                entry["auto"] = json!(true);
            }
            index.push(entry);
        }
        self.json(&format!("{dir}/index.json"), &json!({"versions": index}))
    }
    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for member in &self.members {
            match &member.data {
                Data::Json { bytes, .. } => put(&mut writer, &member.name, json_options(), bytes)?,
                Data::Binary(bytes) => put(&mut writer, &member.name, stored, bytes)?,
            }
        }
        Ok(writer.finish().map_err(io_err)?.into_inner())
    }
    /// Compare actual logical members. With `ignore_main_page`, only the main
    /// document's opening-page preference is ignored, never history/branch pages.
    pub fn same_persisted_content(&self, other: &Self, ignore_main_page: bool) -> bool {
        let keep = |m: &&Member| m.name != "thumbnail.png";
        let mut a = self.members.iter().filter(keep);
        let mut b = other.members.iter().filter(keep);
        loop {
            match (a.next(), b.next()) {
                (None, None) => return true,
                (Some(a), Some(b)) if a.name == b.name => {
                    let member_name_is_main = a.name == "document.json";
                    match (&a.data, &b.data) {
                        (Data::Binary(a), Data::Binary(b)) if a == b => {}
                        (Data::Json { value: a, .. }, Data::Json { value: b, .. }) => {
                            let same = if ignore_main_page && member_name_is_main {
                                match (a.as_object(), b.as_object()) {
                                    (Some(a), Some(b)) => {
                                        a.iter().filter(|(k, _)| k.as_str() != "current_page").all(|(k, v)| b.get(k) == Some(v))
                                            && b.keys().filter(|k| k.as_str() != "current_page").all(|k| a.contains_key(k))
                                    }
                                    _ => a == b,
                                }
                            } else {
                                a == b
                            };
                            if !same {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                }
                _ => return false,
            }
        }
    }
}
