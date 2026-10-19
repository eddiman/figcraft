//! Bounded, owned encoded image conversion outside UIKit.
use crate::snapshot::{IMAGE_LIMIT, RgbaImage, SnapshotError};
use image::ImageDecoder;
use std::io::Cursor;

pub const ENCODED_LIMIT: usize = 16 * 1024 * 1024;

pub fn decode(bytes: &[u8]) -> Result<RgbaImage, SnapshotError> {
    if bytes.len() > ENCODED_LIMIT {
        return Err(SnapshotError::TooLarge);
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|_| SnapshotError::InvalidImage)?;
    let Some(format @ (image::ImageFormat::Png | image::ImageFormat::Jpeg)) = reader.format() else {
        return Err(SnapshotError::InvalidImage);
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(IMAGE_LIMIT as u64);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    // The decoder's allocation budget does not cover conversion from grayscale/RGB to RGBA.
    RgbaImage::byte_len(width, height)?;
    limits.reserve(decoder.total_bytes()).map_err(image_error)?;
    decoder.set_limits(limits).map_err(image_error)?;
    let profile = crate::image_profile::read(bytes, format, &mut decoder)?;
    let decoded = image::DynamicImage::from_decoder(decoder).map_err(image_error)?;
    let result = RgbaImage { width, height, bytes: crate::image_profile::into_srgb(decoded, profile)? };
    result.validate()?;
    Ok(result)
}

fn image_error(error: image::ImageError) -> SnapshotError {
    if matches!(error, image::ImageError::Limits(_)) { SnapshotError::TooLarge } else { SnapshotError::InvalidImage }
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
    use image::ImageEncoder;

    #[test]
    fn ipad_clipboard_rejects_rgba_expansion_before_decoding_pixels() {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes).write_image(&vec![0; 8192 * 2049], 8192, 2049, image::ExtendedColorType::L8).unwrap();
        // Keep the header and IDAT framing, but no valid pixel payload.
        bytes.truncate(41);
        assert_eq!(decode(&bytes), Err(SnapshotError::TooLarge));
    }

    #[test]
    fn ipad_clipboard_image_dimensions_and_alpha_roundtrip() {
        let image = RgbaImage { width: 2, height: 1, bytes: vec![10, 20, 30, 0, 100, 110, 120, 128] };
        assert_eq!(decode(&encode(&image).unwrap()), Ok(image));
        assert_eq!(decode(b"not an image"), Err(SnapshotError::InvalidImage));
        assert_eq!(encode(&RgbaImage { width: 2, height: 1, bytes: vec![0; 7] }), Err(SnapshotError::InvalidImage));
    }

    fn tagged_png(profile: Vec<u8>, pixel: &[u8], kind: image::ExtendedColorType) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(profile).unwrap();
        encoder.write_image(pixel, 1, 1, kind).unwrap();
        bytes
    }

    #[test]
    fn ipad_clipboard_p3_icc_converts_and_preserves_straight_alpha() {
        let profile = moxcms::ColorProfile::new_display_p3().encode().unwrap();
        for alpha in [0, 128, 255] {
            let decoded = decode(&tagged_png(profile.clone(), &[200, 80, 60, alpha], image::ExtendedColorType::Rgba8)).unwrap();
            // Display P3 -> XYZ D65 -> sRGB gives (216.37, 68.88, 50.07).
            for (actual, expected) in decoded.bytes.iter().take(3).zip([216u8, 69, 50]) {
                assert!(actual.abs_diff(expected) <= 2, "{:?}", decoded.bytes);
            }
            assert_eq!(decoded.bytes[3], alpha);
            assert_eq!(decode(&encode(&decoded).unwrap()).unwrap(), decoded, "sRGB bytes are not converted twice");
        }
    }

    #[test]
    fn ipad_clipboard_gray_icc_converts_without_losing_alpha() {
        let profile = moxcms::ColorProfile::new_gray_with_gamma(1.0).encode().unwrap();
        let decoded = decode(&tagged_png(profile, &[128, 90], image::ExtendedColorType::La8)).unwrap();
        for channel in &decoded.bytes[..3] {
            assert!(channel.abs_diff(188) <= 2, "{:?}", decoded.bytes);
        }
        assert_eq!(decoded.bytes[3], 90);
    }

    #[test]
    fn ipad_clipboard_jpeg_icc_converts_to_srgb() {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 100);
        encoder.set_icc_profile(moxcms::ColorProfile::new_display_p3().encode().unwrap()).unwrap();
        encoder.write_image(&[200, 80, 60], 1, 1, image::ExtendedColorType::Rgb8).unwrap();
        let decoded = decode(&bytes).unwrap();
        for (actual, expected) in decoded.bytes.iter().take(3).zip([216u8, 69, 50]) {
            assert!(actual.abs_diff(expected) <= 4, "{:?}", decoded.bytes);
        }
        assert_eq!(decoded.bytes[3], 255);
    }

    #[test]
    fn ipad_clipboard_invalid_or_mismatched_icc_is_not_assumed_srgb() {
        assert_eq!(
            decode(&tagged_png(b"invalid profile".to_vec(), &[10, 20, 30, 255], image::ExtendedColorType::Rgba8)),
            Err(SnapshotError::InvalidImage)
        );
        let profile = moxcms::ColorProfile::new_gray_with_gamma(1.0).encode().unwrap();
        assert_eq!(decode(&tagged_png(profile, &[10, 20, 30, 255], image::ExtendedColorType::Rgba8)), Err(SnapshotError::InvalidImage));
    }

    #[test]
    fn ipad_clipboard_png_gamma_is_converted() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_source_gamma(png::ScaledFloat::new(1.0));
            encoder.write_header().unwrap().write_image_data(&[128, 128, 128, 99]).unwrap();
        }
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.bytes, [188, 188, 188, 99]);
    }

    #[test]
    fn ipad_clipboard_cicp_precedes_icc_and_rejects_unsupported_hdr() {
        for (primaries, transfer, expected) in [(12, 13, Some([216u8, 69, 50, 77])), (1, 13, Some([200, 80, 60, 77])), (12, 16, None)] {
            let mut bytes = Vec::new();
            {
                let mut info = png::Info::with_size(1, 1);
                info.color_type = png::ColorType::Rgba;
                info.icc_profile = Some(moxcms::ColorProfile::new_srgb().encode().unwrap().into());
                let mut writer = png::Encoder::with_info(&mut bytes, info).unwrap().write_header().unwrap();
                writer.write_chunk(png::chunk::ChunkType(*b"cICP"), &[primaries, transfer, 0, 1]).unwrap();
                writer.write_image_data(&[200, 80, 60, 77]).unwrap();
            }
            match expected {
                Some(expected) => {
                    let decoded = decode(&bytes).unwrap();
                    for (actual, expected) in decoded.bytes.iter().zip(expected) {
                        assert!(actual.abs_diff(expected) <= 1, "primaries={primaries}: {:?}", decoded.bytes);
                    }
                }
                None => assert_eq!(decode(&bytes), Err(SnapshotError::InvalidImage)),
            }
        }
    }

    #[test]
    fn ipad_clipboard_metadata_and_native_pixel_allocations_are_bounded() {
        let mut profile = moxcms::ColorProfile::new_srgb().encode().unwrap();
        profile.resize(4 * 1024 * 1024 + 1, 0);
        assert_eq!(
            decode(&tagged_png(profile, &[10, 20, 30, 255], image::ExtendedColorType::Rgba8)),
            Err(SnapshotError::InvalidImage),
            "discarded over-budget ICC metadata must not silently become sRGB"
        );
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes).write_image(&vec![0; 4096 * 2049 * 8], 4096, 2049, image::ExtendedColorType::Rgba16).unwrap();
        assert_eq!(decode(&bytes), Err(SnapshotError::TooLarge), "16-bit decode allocation exceeds the budget even though RGBA8 fits");
    }
}
