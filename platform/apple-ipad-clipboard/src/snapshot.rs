//! Owned clipboard data. This module has no UIKit dependency.
pub const TEXT_LIMIT: usize = 8 * 1024 * 1024;
pub const IMAGE_LIMIT: usize = 64 * 1024 * 1024;
pub const IMAGE_AXIS_LIMIT: u32 = 8192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    Denied,
    Unavailable,
    Changed,
    TooLarge,
    InvalidImage,
    InvalidToken,
    Panicked,
    Reentrant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub token: String,
    pub generation: u64,
}

impl Identity {
    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.token.len() != 36
            || !self.token.bytes().enumerate().all(|(i, byte)| if matches!(i, 8 | 13 | 18 | 23) { byte == b'-' } else { byte.is_ascii_hexdigit() })
        {
            return Err(SnapshotError::InvalidToken);
        }
        Ok(())
    }
}

/// Straight, unpremultiplied sRGB RGBA, with no row padding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

impl RgbaImage {
    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.width == 0 || self.height == 0 {
            return Err(SnapshotError::InvalidImage);
        }
        if self.width > IMAGE_AXIS_LIMIT || self.height > IMAGE_AXIS_LIMIT {
            return Err(SnapshotError::TooLarge);
        }
        let expected = usize::try_from(self.width)
            .ok()
            .and_then(|w| usize::try_from(self.height).ok().and_then(|h| w.checked_mul(h)))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(SnapshotError::TooLarge)?;
        if expected > IMAGE_LIMIT {
            return Err(SnapshotError::TooLarge);
        }
        if self.bytes.len() != expected {
            return Err(SnapshotError::InvalidImage);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub text: Option<String>,
    pub image: Option<RgbaImage>,
    pub identity: Option<Identity>,
}

impl Content {
    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.text.as_ref().is_some_and(|text| text.len() > TEXT_LIMIT) {
            return Err(SnapshotError::TooLarge);
        }
        if let Some(image) = &self.image {
            image.validate()?;
        }
        if let Some(identity) = &self.identity {
            identity.validate()?;
        }
        // Separate text/image bounds imply the combined 72 MiB budget.
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Content(Content),
    Empty,
    Unsupported,
    Failed(SnapshotError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipad_clipboard_image_dimensions() {
        let valid = RgbaImage { width: 2, height: 3, bytes: vec![0; 24] };
        assert_eq!(valid.validate(), Ok(()));
        for (width, height, bytes, expected) in [
            (0, 1, 0, SnapshotError::InvalidImage),
            (1, 0, 0, SnapshotError::InvalidImage),
            (2, 3, 23, SnapshotError::InvalidImage),
            (2, 3, 25, SnapshotError::InvalidImage),
            (u32::MAX, u32::MAX, 0, SnapshotError::TooLarge),
            (8192, 8192, 0, SnapshotError::TooLarge),
        ] {
            assert_eq!(RgbaImage { width, height, bytes: vec![0; bytes] }.validate(), Err(expected));
        }
    }

    #[test]
    fn ipad_clipboard_utf8_and_token_bounds() {
        let mut content = Content {
            text: Some("é".repeat(TEXT_LIMIT / 2)),
            image: None,
            identity: Some(Identity { token: "01234567-89AB-CDEF-0123-456789abcdef".into(), generation: 1 }),
        };
        assert_eq!(content.validate(), Ok(()));
        content.text.as_mut().unwrap().push('a');
        assert_eq!(content.validate(), Err(SnapshotError::TooLarge));
        content.text = None;
        content.identity.as_mut().unwrap().token.replace_range(8..9, "X");
        assert_eq!(content.validate(), Err(SnapshotError::InvalidToken));
    }
}
