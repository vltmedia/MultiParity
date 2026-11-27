//! Thumbnails and previews: decode an image (TGA, TIFF, EXR and HDR included, float images tone
//! mapped) or the preview Blender saves inside a .blend, and shrink it to JPEGs. Used by the
//! server for published files and by the daemon for files in a folder on this computer.

use std::io::Cursor;

use image::{DynamicImage, ImageFormat, RgbImage};

/// Long side of the list thumbnail and of the preview, in pixels.
pub const SMALL: u32 = 256;
pub const LARGE: u32 = 1280;
/// Files bigger than this are not read for a thumbnail.
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Image(ImageFormat),
    Blend,
}

/// What a path can be drawn from, by its extension.
pub fn source_of(path: &str) -> Option<Source> {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    Some(match ext.as_str() {
        "png" => Source::Image(ImageFormat::Png),
        "jpg" | "jpeg" => Source::Image(ImageFormat::Jpeg),
        "gif" => Source::Image(ImageFormat::Gif),
        "webp" => Source::Image(ImageFormat::WebP),
        "bmp" => Source::Image(ImageFormat::Bmp),
        "tga" => Source::Image(ImageFormat::Tga),
        "tif" | "tiff" => Source::Image(ImageFormat::Tiff),
        "exr" => Source::Image(ImageFormat::OpenExr),
        "hdr" => Source::Image(ImageFormat::Hdr),
        "blend" => Source::Blend,
        _ => return None,
    })
}

pub struct Drawn {
    pub width: u32,
    pub height: u32,
    pub small: Vec<u8>,
    pub large: Vec<u8>,
}

/// Decode and shrink. `Ok(None)` when the file holds nothing to show.
pub fn draw(source: Source, bytes: &[u8]) -> Result<Option<Drawn>, String> {
    let img = match source {
        Source::Image(fmt) => {
            let mut r = image::ImageReader::with_format(Cursor::new(bytes), fmt);
            r.no_limits();
            let img = r.decode().map_err(|e| format!("could not read the image: {e}"))?;
            Some(to_display(img))
        }
        Source::Blend => blend_preview(bytes)?,
    };
    let Some(img) = img else { return Ok(None) };
    let (width, height) = (img.width(), img.height());
    Ok(Some(Drawn { width, height, small: jpeg(&img, SMALL)?, large: jpeg(&img, LARGE)? }))
}

/// 8-bit RGB for display: float images (EXR, HDR) are tone mapped, transparency is laid on a
/// light grey.
fn to_display(img: DynamicImage) -> RgbImage {
    let float = matches!(img, DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_));
    let rgba = img.to_rgba32f();
    let mut out = RgbImage::new(rgba.width(), rgba.height());
    for (o, p) in out.pixels_mut().zip(rgba.pixels()) {
        let a = p[3].clamp(0.0, 1.0);
        let ch = |c: f32| {
            let v = if float { (c.max(0.0) / (1.0 + c.max(0.0))).powf(1.0 / 2.2) } else { c.clamp(0.0, 1.0) };
            let v = v * a + 0.85 * (1.0 - a);
            (v * 255.0).round() as u8
        };
        *o = image::Rgb([ch(p[0]), ch(p[1]), ch(p[2])]);
    }
    out
}

fn jpeg(img: &RgbImage, max: u32) -> Result<Vec<u8>, String> {
    let small = if img.width() > max || img.height() > max {
        DynamicImage::ImageRgb8(img.clone()).resize(max, max, image::imageops::FilterType::Triangle).to_rgb8()
    } else {
        img.clone()
    };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 84).encode_image(&small).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The preview Blender keeps in a `.blend` (its `TEST` block), if it saved one. Handles
/// zstd-compressed files (Blender 3.0+) and both the classic and the Blender 5 block headers.
pub fn blend_preview(bytes: &[u8]) -> Result<Option<RgbImage>, String> {
    let owned;
    let data: &[u8] = if bytes.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        owned = zstd::decode_all(Cursor::new(bytes)).map_err(|e| format!("could not unpack the .blend: {e}"))?;
        &owned
    } else if bytes.starts_with(&[0x1F, 0x8B]) {
        return Err("gzip-compressed .blend files (Blender 2.x \"Compress\") are not read yet".into());
    } else {
        bytes
    };
    if !data.starts_with(b"BLENDER") {
        return Err("not a Blender file".into());
    }
    // Classic header: "BLENDER" + '_' (4-byte pointers) or '-' (8) + 'v'/'V' + "300" = 12 bytes.
    // Blender 5 header: "BLENDER" + "17-01v" + "0500" = 17 bytes, 8-byte pointers, large blocks.
    let (mut pos, ptr, little, large) = if data.len() > 17 && &data[7..9] == b"17" && data[9] == b'-' {
        (17usize, 8usize, data[12] == b'v', true)
    } else if data.len() > 12 {
        (12usize, if data[7] == b'-' { 8 } else { 4 }, data[8] == b'v', false)
    } else {
        return Err("the .blend is cut short".into());
    };
    let u32_at = |p: usize| -> Option<u32> {
        let b: [u8; 4] = data.get(p..p + 4)?.try_into().ok()?;
        Some(if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    };
    let u64_at = |p: usize| -> Option<u64> {
        let b: [u8; 8] = data.get(p..p + 8)?.try_into().ok()?;
        Some(if little { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) })
    };
    loop {
        let code = data.get(pos..pos + 4).ok_or("the .blend ended without a preview")?;
        // Block header: code, then (classic) len u32, old pointer, sdna u32, count u32;
        // (Blender 5) sdna u32, old pointer u64, len u64, count u64.
        let (len, head) = if large { (u64_at(pos + 16).ok_or("bad block")? as usize, 4 + 4 + 8 + 8 + 8) } else { (u32_at(pos + 4).ok_or("bad block")? as usize, 4 + 4 + ptr + 4 + 4) };
        let body = pos + head;
        if code == b"TEST" {
            let w = u32_at(body).ok_or("bad preview")? as usize;
            let h = u32_at(body + 4).ok_or("bad preview")? as usize;
            let px = data.get(body + 8..body + 8 + w * h * 4).ok_or("the preview is cut short")?;
            if w == 0 || h == 0 {
                return Ok(None);
            }
            let mut img = RgbImage::new(w as u32, h as u32);
            for y in 0..h {
                for x in 0..w {
                    let i = (y * w + x) * 4;
                    let a = px[i + 3] as f32 / 255.0;
                    let ch = |c: u8| ((c as f32) * a + 217.0 * (1.0 - a)).round() as u8;
                    // Rows are stored bottom up.
                    img.put_pixel(x as u32, (h - 1 - y) as u32, image::Rgb([ch(px[i]), ch(px[i + 1]), ch(px[i + 2])]));
                }
            }
            return Ok(Some(img));
        }
        if code == b"ENDB" || code == b"DNA1" {
            // The preview, when there is one, comes before the data blocks end.
            return Ok(None);
        }
        pos = body.checked_add(len).ok_or("bad block")?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny classic .blend: header, a TEST block with a 2x1 preview, ENDB.
    fn fake_blend(ptr8: bool) -> Vec<u8> {
        let mut b = b"BLENDER".to_vec();
        b.push(if ptr8 { b'-' } else { b'_' });
        b.extend(b"v300");
        let ptr = if ptr8 { 8 } else { 4 };
        let mut body = Vec::new();
        body.extend(2u32.to_le_bytes());
        body.extend(1u32.to_le_bytes());
        body.extend([255, 0, 0, 255, 0, 0, 255, 255]);
        b.extend(b"TEST");
        b.extend((body.len() as u32).to_le_bytes());
        b.extend(vec![0u8; ptr]);
        b.extend(0u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(body);
        b.extend(b"ENDB");
        b.extend(0u32.to_le_bytes());
        b.extend(vec![0u8; ptr + 8]);
        b
    }

    #[test]
    fn reads_blend_previews() {
        for ptr8 in [false, true] {
            let img = blend_preview(&fake_blend(ptr8)).unwrap().unwrap();
            assert_eq!((img.width(), img.height()), (2, 1));
            assert_eq!(img.get_pixel(0, 0).0, [255, 0, 0]);
            assert_eq!(img.get_pixel(1, 0).0, [0, 0, 255]);
        }
        let z = zstd::encode_all(Cursor::new(fake_blend(true)), 3).unwrap();
        assert!(blend_preview(&z).unwrap().is_some());
    }

    /// Real files: MP_TEST_BLENDS=a.blend;b.blend cargo test -p mp-core real_blends -- --ignored
    /// Writes each preview next to its file as .preview.jpg.
    #[test]
    #[ignore]
    fn real_blends() {
        for f in std::env::var("MP_TEST_BLENDS").unwrap_or_default().split(';').filter(|x| !x.is_empty()) {
            let bytes = std::fs::read(f).unwrap();
            let d = draw(Source::Blend, &bytes).unwrap_or_else(|e| panic!("{f}: {e}")).unwrap_or_else(|| panic!("{f}: no preview"));
            println!("{f}: {}x{}", d.width, d.height);
            std::fs::write(format!("{f}.preview.jpg"), d.large).unwrap();
        }
    }

    #[test]
    fn no_preview_is_none() {
        let mut b = b"BLENDER-v300".to_vec();
        b.extend(b"ENDB");
        b.extend(vec![0u8; 20]);
        assert!(blend_preview(&b).unwrap().is_none());
        assert!(blend_preview(b"nope").is_err());
    }

    #[test]
    fn draws_png_and_hdr() {
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(600, 300, image::Rgba([10, 200, 30, 255]))).write_to(&mut Cursor::new(&mut png), ImageFormat::Png).unwrap();
        let d = draw(Source::Image(ImageFormat::Png), &png).unwrap().unwrap();
        assert_eq!((d.width, d.height), (600, 300));
        let small = image::load_from_memory(&d.small).unwrap();
        assert_eq!((small.width(), small.height()), (256, 128));
        let mut exr = Vec::new();
        DynamicImage::ImageRgba32F(image::Rgba32FImage::from_pixel(4, 4, image::Rgba([4.0, 1.0, 0.25, 1.0]))).write_to(&mut Cursor::new(&mut exr), ImageFormat::OpenExr).unwrap();
        assert!(draw(Source::Image(ImageFormat::OpenExr), &exr).unwrap().is_some());
    }
}
