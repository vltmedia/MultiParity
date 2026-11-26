//! Blob storage for chunks. One interface over a local directory (also used for mounted network
//! paths) and any S3-compatible service.

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
    S3 { bucket: String, endpoint: Option<String>, region: Option<String>, prefix: Option<String> },
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
            StorageConfig::S3 { bucket, endpoint, region, prefix } => {
                let mut b = AmazonS3Builder::from_env().with_bucket_name(bucket);
                if let Some(e) = endpoint {
                    b = b.with_endpoint(e).with_allow_http(e.starts_with("http://")).with_virtual_hosted_style_request(false);
                }
                b = b.with_region(region.clone().unwrap_or_else(|| "us-east-1".into()));
                (Arc::new(b.build()?), prefix.clone().unwrap_or_default().trim_matches('/').to_string())
            }
        };
        Ok(BlobStore { inner, prefix, description: cfg.describe() })
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

    /// Total bytes stored under chunks/ (for the admin overview). Lists every object, so it is
    /// called on demand rather than on every page load.
    pub async fn total_bytes(&self) -> Result<u64, object_store::Error> {
        let prefix = if self.prefix.is_empty() { ObjPath::from("chunks") } else { ObjPath::from(format!("{}/chunks", self.prefix)) };
        let items: Vec<_> = self.inner.list(Some(&prefix)).try_collect().await?;
        Ok(items.iter().map(|m| m.size as u64).sum())
    }
}
