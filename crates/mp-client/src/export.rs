//! Exporting a folder's project to this computer: the branch's published files, at its latest
//! change, into a folder or one .zip. Files this computer already has, unchanged, are copied
//! from the synced folder; the rest are downloaded. Unpublished edits are never included.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::api::Api;
use crate::ops::write_manifest_to;
use crate::progress;
use crate::workspace::{mtime_of, Workspace};

#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    /// The folder or .zip written.
    pub path: String,
    pub files: usize,
    pub bytes: u64,
    /// How many came from the synced folder instead of the server.
    pub copied_locally: usize,
    pub change: i64,
}

/// A name for the export: project, branch and change.
pub fn export_name(ws: &Workspace, change: i64) -> String {
    let clean = |t: &str| t.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect::<String>();
    format!("{}-{}-change{}", clean(&ws.config.project_name), clean(&ws.config.branch_name).to_lowercase(), change)
}

/// Export into `into` (a folder): as `<name>/` with the files, or as `<name>.zip`.
pub async fn export(api: &Api, ws: &Workspace, into: &Path, zip: bool) -> Result<ExportReport> {
    progress::phase("Checking what to export", 0, 0);
    let tree = api.tree(&ws.config.project_id, &ws.config.branch_id, 0).await?;
    let entries: Vec<_> = tree.entries.iter().filter(|e| e.file_hash.is_some() && e.manifest_hash.is_some()).collect();
    let total: u64 = entries.iter().map(|e| e.size.max(0) as u64).sum();
    // Inside the synced folder, the export would show up as new files to publish. Compared as
    // full paths (the folder is kept canonical, \\?\D:\…), case-blind as Windows and macOS are.
    let full = |p: &Path| -> String {
        let p = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
        // The nearest part of the path that exists, made canonical, plus the rest.
        let mut base = p.clone();
        let mut rest = Vec::new();
        while !base.exists() {
            match (base.file_name().map(|n| n.to_owned()), base.parent().map(|d| d.to_path_buf())) {
                (Some(n), Some(d)) => { rest.push(n); base = d; }
                _ => break,
            }
        }
        let mut c = std::fs::canonicalize(&base).unwrap_or(base);
        for n in rest.into_iter().rev() { c.push(n); }
        let s = c.display().to_string().replace('/', "\\");
        s.strip_prefix(r"\\?\").unwrap_or(&s).trim_end_matches('\\').to_lowercase()
    };
    let (into_s, root_s) = (full(into), full(&ws.root));
    anyhow::ensure!(
        into_s != root_s && !into_s.starts_with(&format!("{root_s}\\")),
        "Choose a folder outside {}: an export inside it would show up as new files to publish.",
        ws.root.display().to_string().trim_start_matches(r"\\?\")
    );
    std::fs::create_dir_all(into).with_context(|| format!("creating {}", into.display()))?;
    let name = export_name(ws, tree.head);
    let out = if zip { into.join(format!("{name}.zip")) } else { into.join(&name) };
    anyhow::ensure!(!out.exists(), "{} already exists. Choose another folder, or move it first.", out.display());
    progress::phase("Exporting", entries.len() as u64, total);

    let mut report = ExportReport { path: out.display().to_string(), files: 0, bytes: 0, copied_locally: 0, change: tree.head };
    // A file is taken from the synced folder when it is exactly the published version.
    let local_copy = |path: &str, file_hash: &str| -> Option<PathBuf> {
        let ix = ws.index.files.get(path)?;
        if ix.file_hash != file_hash {
            return None;
        }
        let abs = ws.abs(path);
        let m = std::fs::metadata(&abs).ok()?;
        (m.is_file() && m.len() == ix.size && mtime_of(&m) == ix.mtime).then_some(abs)
    };

    if zip {
        let tmp = out.with_extension("zip.partial");
        let f = std::fs::File::create(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
        let mut z = zip::ZipWriter::new(std::io::BufWriter::with_capacity(1 << 20, f));
        let scratch = ws.tmp_dir()?;
        let result: Result<()> = async {
            for e in &entries {
                progress::file(&e.path);
                let size = e.size.max(0) as u64;
                let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(size >= 0xFFFF_0000);
                z.start_file(e.path.as_str(), opts)?;
                let (src, downloaded) = match local_copy(&e.path, e.file_hash.as_deref().unwrap()) {
                    Some(p) => (p, false),
                    None => {
                        let t = scratch.join(format!("export-{}", uuid::Uuid::new_v4()));
                        let m = api.manifest(&ws.config.project_id, e.manifest_hash.as_deref().unwrap()).await?;
                        write_manifest_to(api, ws, &e.path, &t, &m).await?;
                        (t, true)
                    }
                };
                let mut r = std::fs::File::open(&src)?;
                let mut buf = vec![0u8; 1 << 20];
                loop {
                    let n = r.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    z.write_all(&buf[..n])?;
                    if !downloaded {
                        progress::bytes(n as u64);
                    }
                }
                if downloaded {
                    let _ = std::fs::remove_file(&src);
                } else {
                    report.copied_locally += 1;
                }
                report.files += 1;
                report.bytes += size;
                progress::file_done();
            }
            Ok(())
        }
        .await;
        if let Err(e) = result {
            drop(z);
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        z.finish()?.flush()?;
        std::fs::rename(&tmp, &out)?;
    } else {
        for e in &entries {
            progress::file(&e.path);
            let dest = out.join(e.path.replace('/', std::path::MAIN_SEPARATOR_STR));
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir)?;
            }
            match local_copy(&e.path, e.file_hash.as_deref().unwrap()) {
                Some(src) => {
                    let n = std::fs::copy(&src, &dest).with_context(|| format!("copying {}", e.path))?;
                    progress::bytes(n);
                    report.copied_locally += 1;
                }
                None => {
                    let m = api.manifest(&ws.config.project_id, e.manifest_hash.as_deref().unwrap()).await?;
                    write_manifest_to(api, ws, &e.path, &dest, &m).await?;
                }
            }
            report.files += 1;
            report.bytes += e.size.max(0) as u64;
            progress::file_done();
        }
    }
    Ok(report)
}
