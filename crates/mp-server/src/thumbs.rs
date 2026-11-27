//! Thumbnails and previews of published files.
//!
//! Publishing queues every file that can be drawn; a background worker reads it from storage
//! and keeps two JPEGs in the database: `small` (256 px, for lists) and `large` (1280 px, for
//! the preview). Drawn: common image formats including TGA, TIFF, EXR and HDR (tone mapped),
//! and Blender files, from the preview Blender saves inside them.

use std::io::Cursor;
use std::time::Duration;

use image::{DynamicImage, ImageFormat, RgbImage};
use sqlx::Row;
use uuid::Uuid;

use crate::AppState;

pub const SMALL: u32 = 256;
pub const LARGE: u32 = 1280;
/// Files bigger than this are not read for a thumbnail.
const MAX_BYTES: i64 = 512 * 1024 * 1024;

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

/// Queue files of a project for drawing (those with a drawable extension and no row yet).
pub async fn enqueue(db: &sqlx::PgPool, project: Uuid, files: &[(String, String, String, i64)]) {
    for (path, file_hash, manifest_hash, size) in files {
        if source_of(path).is_none() || *size > MAX_BYTES {
            continue;
        }
        let _ = sqlx::query("INSERT INTO thumbnails (file_hash, project_id, manifest_hash, path) VALUES ($1,$2,$3,$4) ON CONFLICT (file_hash) DO NOTHING")
            .bind(file_hash)
            .bind(project)
            .bind(manifest_hash)
            .bind(path)
            .execute(db)
            .await;
    }
}

/// Queue drawable files on any branch that have no thumbnail yet (new publishes, merges, and
/// files published before thumbnails existed). Returns how many were queued.
async fn find_new(db: &sqlx::PgPool) -> crate::error::ApiResult<usize> {
    let rows = sqlx::query(
        "SELECT DISTINCT ON (t.file_hash) t.path, t.file_hash, t.manifest_hash, t.size, b.project_id
         FROM tree_entries t JOIN branches b ON b.id = t.branch_id
         LEFT JOIN thumbnails th ON th.file_hash = t.file_hash
         WHERE th.file_hash IS NULL AND t.file_hash IS NOT NULL AND t.manifest_hash IS NOT NULL
           AND lower(t.path) ~ '[.](png|jpe?g|gif|webp|bmp|tga|tiff?|exr|hdr|blend)$'
         LIMIT 500",
    )
    .fetch_all(db)
    .await?;
    let n = rows.len();
    for r in rows {
        let project: Uuid = r.get("project_id");
        enqueue(db, project, &[(r.get("path"), r.get("file_hash"), r.get("manifest_hash"), r.get("size"))]).await;
    }
    Ok(n)
}

/// Draw queued files, one at a time, for as long as the server runs.
pub fn spawn_worker(s: AppState) {
    tokio::spawn(async move {
        loop {
            match work_one(&s).await {
                Ok(true) => continue,
                Ok(false) => {
                    if !matches!(find_new(&s.db).await, Ok(n) if n > 0) {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e.message, "thumbnail worker");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });
}

/// Draw the oldest queued file. `Ok(false)` when there was nothing to do.
async fn work_one(s: &AppState) -> crate::error::ApiResult<bool> {
    let Some(r) = sqlx::query(
        "UPDATE thumbnails SET status = 'pending', done_at = now() WHERE file_hash = (
           SELECT file_hash FROM thumbnails WHERE status = 'pending' AND (done_at IS NULL OR done_at < now() - interval '10 minutes')
           ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED)
         RETURNING file_hash, project_id, manifest_hash, path",
    )
    .fetch_optional(&s.db)
    .await?
    else {
        return Ok(false);
    };
    let file_hash: String = r.get("file_hash");
    let path: String = r.get("path");
    let manifest: String = r.get("manifest_hash");
    let project: Option<Uuid> = r.get("project_id");
    let result = async {
        let project = project.ok_or_else(|| "the project is gone".to_string())?;
        let mut conn = s.db.acquire().await.map_err(|e| e.to_string())?;
        let ps = crate::chunks::ProjectStore::load(&mut conn, project).await.map_err(|e| e.message)?;
        let bytes = crate::branches::read_blob(s, &mut conn, &ps, &manifest).await.map_err(|e| e.message)?;
        drop(conn);
        let source = source_of(&path).ok_or_else(|| "not a drawable file".to_string())?;
        tokio::task::spawn_blocking(move || draw(source, &bytes)).await.map_err(|e| e.to_string())?
    }
    .await;
    let q = match result {
        Ok(Some(d)) => sqlx::query("UPDATE thumbnails SET status = 'ready', width = $2, height = $3, small = $4, large = $5, error = NULL, done_at = now() WHERE file_hash = $1")
            .bind(&file_hash)
            .bind(d.width as i32)
            .bind(d.height as i32)
            .bind(d.small)
            .bind(d.large),
        Ok(None) => sqlx::query("UPDATE thumbnails SET status = 'none', done_at = now() WHERE file_hash = $1").bind(&file_hash),
        Err(e) => {
            tracing::info!(path = %path, error = %e, "no thumbnail");
            sqlx::query("UPDATE thumbnails SET status = 'failed', error = $2, done_at = now() WHERE file_hash = $1").bind(&file_hash).bind(e)
        }
    };
    q.execute(&s.db).await?;
    Ok(true)
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

    /// Real files: MP_TEST_BLENDS=a.blend;b.blend cargo test -p mp-server real_blends -- --ignored
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
