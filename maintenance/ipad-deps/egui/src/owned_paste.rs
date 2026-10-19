//! Owned, bounded outcomes for an explicitly configured native paste reader.
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PasteError {
    Denied,
    Unavailable,
    Changed,
    TooLarge,
    InvalidContent,
    Panicked,
    Reentrant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PasteIdentity {
    pub token: String,
    pub generation: u64,
}

impl PasteIdentity {
    pub fn validate(&self) -> Result<(), PasteError> {
        if self.token.len() != 36
            || !self.token.bytes().enumerate().all(|(i, b)| {
                if matches!(i, 8 | 13 | 18 | 23) { b == b'-' } else { b.is_ascii_hexdigit() }
            })
        {
            return Err(PasteError::InvalidContent);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PasteImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl PasteImage {
    pub fn validate(&self) -> Result<(), PasteError> {
        if self.width == 0 || self.height == 0 {
            return Err(PasteError::InvalidContent);
        }
        let bytes = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .and_then(|n| n.checked_mul(4))
            .ok_or(PasteError::TooLarge)?;
        if self.width > 8192 || self.height > 8192 || bytes > 64 * 1024 * 1024 {
            return Err(PasteError::TooLarge);
        }
        if usize::try_from(bytes).ok() != Some(self.rgba.len()) {
            return Err(PasteError::InvalidContent);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct PasteContent {
    pub text: Option<String>,
    pub image: Option<PasteImage>,
    pub identity: Option<PasteIdentity>,
}

impl PasteContent {
    pub fn validate(&self) -> Result<(), PasteError> {
        if self
            .text
            .as_ref()
            .is_some_and(|s| s.len() > crate::TEXT_CLIPBOARD_LIMIT)
        {
            return Err(PasteError::TooLarge);
        }
        if let Some(image) = &self.image {
            image.validate()?;
        }
        if let Some(identity) = &self.identity {
            identity.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PasteOutcome {
    Content(PasteContent),
    Empty,
    Unsupported,
    Failed(PasteError),
}

pub type PasteReader = Arc<dyn Fn() -> PasteOutcome + Send + Sync>;
