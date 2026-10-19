//! Clipboard pixels must be sRGB before the owned snapshot leaves this boundary.
use crate::snapshot::SnapshotError;
use image::{DynamicImage, ImageDecoder, ImageFormat};
use moxcms::{Chromaticity, ColorPrimaries, ColorProfile, DataColorSpace, Layout, ParsingOptions, ToneReprCurve, TransformOptions, XyY};
use std::io::Cursor;

const ICC_LIMIT: usize = 4 * 1024 * 1024;

fn icc(bytes: &[u8]) -> Result<ColorProfile, SnapshotError> {
    if bytes.len() > ICC_LIMIT {
        return Err(SnapshotError::TooLarge);
    }
    ColorProfile::new_from_slice_with_options(bytes, ParsingOptions { max_profile_size: ICC_LIMIT + 1, ..Default::default() })
        .map_err(|_| SnapshotError::InvalidImage)
}

pub(crate) fn read(bytes: &[u8], format: ImageFormat, decoder: &mut impl ImageDecoder) -> Result<Option<ColorProfile>, SnapshotError> {
    if format != ImageFormat::Png {
        return decoder.icc_profile().map_err(|_| SnapshotError::InvalidImage)?.map(|bytes| icc(&bytes)).transpose();
    }
    let reader = png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: ICC_LIMIT })
        .read_info()
        .map_err(|error| if matches!(error, png::DecodingError::LimitsExceeded) { SnapshotError::TooLarge } else { SnapshotError::InvalidImage })?;
    let info = reader.info();
    // png may discard malformed or over-budget ancillary chunks without failing read_info.
    // Never reinterpret explicitly tagged pixels as untagged sRGB after such a discard.
    let mut offset = 8usize;
    while let Some(header) = bytes.get(offset..offset.saturating_add(8)) {
        let size = u32::from_be_bytes(header.get(..4).ok_or(SnapshotError::InvalidImage)?.try_into().map_err(|_| SnapshotError::InvalidImage)?);
        let kind = header.get(4..8).ok_or(SnapshotError::InvalidImage)?;
        let missing = match kind {
            b"iCCP" => info.icc_profile.is_none(),
            b"cICP" => info.coding_independent_code_points.is_none(),
            b"sRGB" => info.srgb.is_none(),
            b"gAMA" => info.gama_chunk.is_none(),
            b"cHRM" => info.chrm_chunk.is_none(),
            b"IDAT" | b"IEND" => break,
            _ => false,
        };
        if missing {
            return Err(SnapshotError::InvalidImage);
        }
        offset = offset.checked_add(size as usize).and_then(|offset| offset.checked_add(12)).ok_or(SnapshotError::TooLarge)?;
        if offset > bytes.len() {
            return Err(SnapshotError::InvalidImage);
        }
    }
    // cICP takes precedence over legacy PNG color metadata. HDR needs a separately reviewed tone map.
    if let Some(cicp) = info.coding_independent_code_points {
        if cicp.matrix_coefficients != 0 || !cicp.is_video_full_range_image || cicp.transfer_function != 13 {
            return Err(SnapshotError::InvalidImage);
        }
        return match cicp.color_primaries {
            1 => Ok(None),
            12 => Ok(Some(ColorProfile::new_display_p3())),
            _ => Err(SnapshotError::InvalidImage),
        };
    }
    if let Some(bytes) = &info.icc_profile {
        return icc(bytes).map(Some);
    }
    if info.srgb.is_some() {
        return Ok(None);
    }
    if info.gama_chunk.is_none() && info.chrm_chunk.is_none() {
        return Ok(None);
    }
    let mut profile = ColorProfile::new_srgb();
    profile.cicp = None;
    if let Some(chrm) = info.chrm_chunk {
        let xy = |(x, y): (png::ScaledFloat, png::ScaledFloat)| -> Result<Chromaticity, SnapshotError> {
            let (x, y) = (x.into_value(), y.into_value());
            if !x.is_finite() || !y.is_finite() || x < 0.0 || y <= 0.001 || x + y > 1.0 {
                return Err(SnapshotError::InvalidImage);
            }
            Ok(Chromaticity::new(x, y))
        };
        let white = xy(chrm.white)?;
        profile.update_rgb_colorimetry(
            XyY::new(f64::from(white.x), f64::from(white.y), 1.0),
            ColorPrimaries { red: xy(chrm.red)?, green: xy(chrm.green)?, blue: xy(chrm.blue)? },
        );
    }
    if let Some(gamma) = info.gama_chunk {
        let gamma = gamma.into_value();
        if !gamma.is_finite() || !(0.1..=10.0).contains(&gamma) {
            return Err(SnapshotError::InvalidImage);
        }
        let curve = ToneReprCurve::Parametric(vec![1.0 / gamma]);
        (profile.red_trc, profile.green_trc, profile.blue_trc) = (Some(curve.clone()), Some(curve.clone()), Some(curve));
    }
    Ok(Some(profile))
}

pub(crate) fn into_srgb(decoded: DynamicImage, source: Option<ColorProfile>) -> Result<Vec<u8>, SnapshotError> {
    let Some(source) = source else { return Ok(decoded.into_rgba8().into_raw()) };
    let target = ColorProfile::new_srgb();
    match source.color_space {
        DataColorSpace::Rgb => {
            if !source.rgb_to_xyz_matrix().v.iter().flatten().all(|value| value.is_finite()) {
                return Err(SnapshotError::InvalidImage);
            }
            let transform = source
                .create_transform_8bit(Layout::Rgba, &target, Layout::Rgba, TransformOptions::default())
                .map_err(|_| SnapshotError::InvalidImage)?;
            let mut pixels = decoded.into_rgba8().into_raw();
            let mut source_pixels = [0; 4096];
            for chunk in pixels.chunks_mut(source_pixels.len()) {
                let source = source_pixels.get_mut(..chunk.len()).ok_or(SnapshotError::InvalidImage)?;
                source.copy_from_slice(chunk);
                transform.transform(source, chunk).map_err(|_| SnapshotError::InvalidImage)?;
            }
            Ok(pixels)
        }
        DataColorSpace::Gray if !decoded.color().has_color() => {
            let transform = source
                .create_transform_8bit(Layout::GrayAlpha, &target, Layout::Rgba, TransformOptions::default())
                .map_err(|_| SnapshotError::InvalidImage)?;
            let len = crate::snapshot::RgbaImage::byte_len(decoded.width(), decoded.height())?;
            let pixels = decoded.into_luma_alpha8().into_raw();
            let mut output = vec![0; len];
            transform.transform(&pixels, &mut output).map_err(|_| SnapshotError::InvalidImage)?;
            Ok(output)
        }
        _ => Err(SnapshotError::InvalidImage),
    }
}
