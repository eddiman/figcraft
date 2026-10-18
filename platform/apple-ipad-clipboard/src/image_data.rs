//! Bounded, owned encoded image conversion outside UIKit.
use crate::snapshot::{IMAGE_LIMIT, RgbaImage, SnapshotError};
use std::io::Cursor;

pub const ENCODED_LIMIT: usize = 16 * 1024 * 1024;

pub fn decode(bytes: &[u8]) -> Result<RgbaImage, SnapshotError> {
    if bytes.len() > ENCODED_LIMIT {
        return Err(SnapshotError::TooLarge);
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|_| SnapshotError::InvalidImage)?;
    if !matches!(reader.format(), Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)) {
        return Err(SnapshotError::InvalidImage);
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(IMAGE_LIMIT as u64);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| if matches!(error, image::ImageError::Limits(_)) { SnapshotError::TooLarge } else { SnapshotError::InvalidImage })?;
    let result = RgbaImage { width: decoded.width(), height: decoded.height(), bytes: decoded.into_rgba8().into_raw() };
    result.validate()?;
    Ok(result)
}

pub fn encode(image: &RgbaImage) -> Result<Vec<u8>, SnapshotError> {
    image.validate()?;
    let rgba = image::RgbaImage::from_raw(image.width, image.height, image.bytes.clone()).ok_or(SnapshotError::InvalidImage)?;
    let mut output = Cursor::new(Vec::new());
    rgba.write_to(&mut output, image::ImageFormat::Png).map_err(|_| SnapshotError::InvalidImage)?;
    let bytes = output.into_inner();
    if bytes.len() > ENCODED_LIMIT {
        return Err(SnapshotError::TooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipad_clipboard_image_dimensions_and_alpha_roundtrip() {
        let image = RgbaImage { width: 2, height: 1, bytes: vec![10, 20, 30, 0, 100, 110, 120, 128] };
        assert_eq!(decode(&encode(&image).unwrap()), Ok(image));
        assert_eq!(decode(b"not an image"), Err(SnapshotError::InvalidImage));
        assert_eq!(encode(&RgbaImage { width: 2, height: 1, bytes: vec![0; 7] }), Err(SnapshotError::InvalidImage));
    }
}
