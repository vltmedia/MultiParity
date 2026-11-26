//! Blob storage for chunks. One interface over a local directory (also used for mounted network
//! paths) and any S3-compatible service.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::TryStreamExt;
use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, PutPayload};

#[derive(Debug, Clone)]
pub enum StorageConfig {
    /// A directory on this machine, or a mounted share (NFS, SMB). Mounted paths get a health
    /// check at start-up so a missing mount fails loudly instead of filling the container disk.
    Local { path: String, mounted: bool },
    S3 {
        bucket: String,
        endpoint: Option<String>,
        region: Option<String>,
        prefix: Option<String>,
        /// Credentials for this location only; otherwise the usual `AWS_*` variables.
        access_key: Option<(String, String)>,
    },
}

/// The location every server has: the one `MP_STORAGE` describes. Projects use it unless they
/// are given another.
pub const MAIN: &str = "main";

/// Named storage locations: `main` from `MP_STORAGE`, plus any in `MP_STORAGE_LOCATIONS`:
///
/// ```text
/// MP_STORAGE_LOCATIONS="nas=mount:/data/nas archive=s3:bucket=mp-archive;endpoint=https://x.r2.cloudflarestorage.com;region=auto;prefix=mp"
/// ```
///
/// Locations are separated by spaces or commas; each is `name=kind:details`, with `kind` one of
/// `local`, `mount` (a path) or `s3` (`;`-separated settings). An S3 location's credentials come
/// from `MP_STORAGE_<NAME>_ACCESS_KEY_ID` and `_SECRET_ACCESS_KEY`, or the `AWS_*` variables.
pub fn locations_from_env(main: StorageConfig) -> Result<Vec<(String, StorageConfig)>> {
    let mut out = vec![(MAIN.to_string(), main)];
    let spec = std::env::var("MP_STORAGE_LOCATIONS").unwrap_or_default();
    for item in spec.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()) {
        let (name, rest) = item.split_once('=').with_context(|| format!("storage location `{item}` should look like name=kind:details"))?;
        let name = name.trim().to_lowercase();
        anyhow::ensure!(
            !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "storage location name `{name}` may use letters, digits, - and _ (up to 40)"
        );
        anyhow::ensure!(!out.iter().any(|(n, _)| *n == name), "storage location `{name}` is defined twice");
        out.push((name.clone(), parse_spec(&name, rest)?));
    }
    Ok(out)
}

/// One storage spec, `kind:details`: `local:/path`, `mount:/path`, or
/// `s3:bucket=...;endpoint=...;region=...;prefix=...` (credentials from
/// `MP_STORAGE_<NAME>_ACCESS_KEY_ID` / `_SECRET_ACCESS_KEY`, else the `AWS_*` variables).
pub fn parse_spec(name: &str, spec: &str) -> Result<StorageConfig> {
    let (kind, details) = spec.split_once(':').with_context(|| format!("storage `{name}` should be kind:details, like mount:/data/nas"))?;
    Ok(match kind {
        "local" | "mount" => StorageConfig::Local { path: details.to_string(), mounted: kind == "mount" },
        "s3" => {
            let mut kv: BTreeMap<&str, &str> = BTreeMap::new();
            for part in details.split(';').filter(|p| !p.is_empty()) {
                let (k, v) = part.split_once('=').with_context(|| format!("storage `{name}`: `{part}` should be key=value"))?;
                kv.insert(k, v);
            }
            let env = |suffix: &str| std::env::var(format!("MP_STORAGE_{}_{suffix}", name.to_uppercase().replace('-', "_"))).ok();
            StorageConfig::S3 {
                bucket: kv.get("bucket").map(|s| s.to_string()).with_context(|| format!("storage `{name}` needs bucket=…"))?,
                endpoint: kv.get("endpoint").map(|s| s.to_string()),
                region: kv.get("region").map(|s| s.to_string()),
                prefix: kv.get("prefix").map(|s| s.to_string()),
                access_key: env("ACCESS_KEY_ID").zip(env("SECRET_ACCESS_KEY")),
            }
        }
        other => anyhow::bail!("storage `{name}`: kind must be local, mount or s3, not {other}"),
    })
}

/// Every configured location, opened.
pub struct Locations {
    stores: BTreeMap<String, BlobStore>,
}

impl Locations {
    pub async fn open(list: &[(String, StorageConfig)]) -> Result<Self> {
        let mut stores = BTreeMap::new();
        for (name, cfg) in list {
            let s = BlobStore::open(cfg).await.with_context(|| format!("opening storage location `{name}`"))?;
            stores.insert(name.clone(), s);
        }
        Ok(Locations { stores })
    }

    pub fn get(&self, name: &str) -> Option<&BlobStore> {
        self.stores.get(name)
    }

    pub fn main(&self) -> &BlobStore {
        &self.stores[MAIN]
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.stores.keys()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &BlobStore)> {
        self.stores.iter()
    }
}

impl StorageConfig {
    /// From environment: `MP_STORAGE=local|mount|s3`, `MP_STORAGE_PATH`, `MP_S3_BUCKET`,
    /// `MP_S3_ENDPOINT`, `MP_S3_REGION`, `MP_S3_PREFIX` and the usual `AWS_*` credentials.
    pub fn from_env() -> Result<Self> {
        let kind = std::env::var("MP_STORAGE").unwrap_or_else(|_| "local".into());
        match kind.as_str() {
            "local" | "mount" => Ok(StorageConfig::Local {
                path: std::env::var("MP_STORAGE_PATH").unwrap_or_else(|_| "/data/blobs".into()),
                mounted: kind == "mount",
            }),
            "s3" => Ok(StorageConfig::S3 {
                bucket: std::env::var("MP_S3_BUCKET").context("MP_S3_BUCKET is required when MP_STORAGE=s3")?,
                endpoint: std::env::var("MP_S3_ENDPOINT").ok(),
                region: std::env::var("MP_S3_REGION").ok(),
                prefix: std::env::var("MP_S3_PREFIX").ok(),
                access_key: None,
            }),
            other => anyhow::bail!("MP_STORAGE must be local, mount or s3, not {other}"),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            StorageConfig::Local { path, mounted: false } => format!("local directory {path}"),
            StorageConfig::Local { path, mounted: true } => format!("mounted path {path}"),
            StorageConfig::S3 { bucket, endpoint, .. } => {
                format!("S3 bucket {bucket}{}", endpoint.as_ref().map(|e| format!(" at {e}")).unwrap_or_default())
            }
        }
    }
}

#[derive(Clone)]
pub struct BlobStore {
    inner: Arc<dyn ObjectStore>,
    prefix: String,
    pub description: String,
    /// "local", "mount" or "s3".
    pub kind: &'static str,
}

impl BlobStore {
    pub async fn open(cfg: &StorageConfig) -> Result<Self> {
        let (inner, prefix): (Arc<dyn ObjectStore>, String) = match cfg {
            StorageConfig::Local { path, mounted } => {
                if *mounted {
                    let meta = std::fs::metadata(path).with_context(|| format!("the mounted storage path {path} is not reachable"))?;
                    anyhow::ensure!(meta.is_dir(), "the mounted storage path {path} is not a directory");
                    let probe = std::path::Path::new(path).join(".mp-write-test");
                    std::fs::write(&probe, b"ok").with_context(|| format!("cannot write to the mounted storage path {path}"))?;
                    let _ = std::fs::remove_file(probe);
                } else {
                    std::fs::create_dir_all(path).with_context(|| format!("creating storage directory {path}"))?;
                }
                (Arc::new(LocalFileSystem::new_with_prefix(path)?), String::new())
            }
            StorageConfig::S3 { bucket, endpoint, region, prefix, access_key } => {
                let mut b = AmazonS3Builder::from_env().with_bucket_name(bucket);
                if let Some((id, secret)) = access_key {
                    b = b.with_access_key_id(id).with_secret_access_key(secret);
                }
                if let Some(e) = endpoint {
                    b = b.with_endpoint(e).with_allow_http(e.starts_with("http://")).with_virtual_hosted_style_request(false);
                }
                b = b.with_region(region.clone().unwrap_or_else(|| "us-east-1".into()));
                (Arc::new(b.build()?), prefix.clone().unwrap_or_default().trim_matches('/').to_string())
            }
        };
        let kind = match cfg {
            StorageConfig::Local { mounted: false, .. } => "local",
            StorageConfig::Local { mounted: true, .. } => "mount",
            StorageConfig::S3 { .. } => "s3",
        };
        Ok(BlobStore { inner, prefix, description: cfg.describe(), kind })
    }

    fn chunk_path(&self, hash: &str) -> ObjPath {
        let rel = format!("chunks/{}/{}", &hash[..2], hash);
        if self.prefix.is_empty() {
            ObjPath::from(rel)
        } else {
            ObjPath::from(format!("{}/{}", self.prefix, rel))
        }
    }

    pub async fn put_chunk(&self, hash: &str, compressed: Bytes) -> Result<(), object_store::Error> {
        self.inner.put(&self.chunk_path(hash), PutPayload::from(compressed)).await.map(|_| ())
    }

    pub async fn get_chunk(&self, hash: &str) -> Result<Bytes, object_store::Error> {
        self.inner.get(&self.chunk_path(hash)).await?.bytes().await
    }

    pub async fn has_chunk(&self, hash: &str) -> Result<bool, object_store::Error> {
        match self.inner.head(&self.chunk_path(hash)).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn delete_chunk(&self, hash: &str) -> Result<(), object_store::Error> {
        match self.inner.delete(&self.chunk_path(hash)).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn raw_path(&self, rel: &str) -> ObjPath {
        if self.prefix.is_empty() {
            ObjPath::from(rel)
        } else {
            ObjPath::from(format!("{}/{}", self.prefix, rel))
        }
    }

    /// Plain objects by relative path (backups use these alongside chunk copies).
    pub async fn put_raw(&self, rel: &str, data: Bytes) -> Result<(), object_store::Error> {
        self.inner.put(&self.raw_path(rel), PutPayload::from(data)).await.map(|_| ())
    }

    pub async fn get_raw(&self, rel: &str) -> Result<Bytes, object_store::Error> {
        self.inner.get(&self.raw_path(rel)).await?.bytes().await
    }

    pub async fn delete_raw(&self, rel: &str) -> Result<(), object_store::Error> {
        match self.inner.delete(&self.raw_path(rel)).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Every object under `rel` (relative path, size).
    pub async fn list_raw(&self, rel: &str) -> Result<Vec<(String, u64)>, object_store::Error> {
        let base = self.raw_path(rel);
        let strip = if self.prefix.is_empty() { String::new() } else { format!("{}/", self.prefix) };
        let items: Vec<_> = self.inner.list(Some(&base)).try_collect().await?;
        Ok(items.into_iter().map(|m| (m.location.as_ref().trim_start_matches(strip.as_str()).to_string(), m.size as u64)).collect())
    }

    /// Total bytes stored under chunks/ (for the admin overview). Lists every object, so it is
    /// called on demand rather than on every page load.
    pub async fn total_bytes(&self) -> Result<u64, object_store::Error> {
        let prefix = if self.prefix.is_empty() { ObjPath::from("chunks") } else { ObjPath::from(format!("{}/chunks", self.prefix)) };
        let items: Vec<_> = self.inner.list(Some(&prefix)).try_collect().await?;
        Ok(items.iter().map(|m| m.size as u64).sum())
    }
}
