use serde::{Deserialize, Serialize};

/// One chunk in a manifest. Serialised as `[hash, len]` to keep large manifests compact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkRef {
    pub hash: String,
    pub len: u64,
}

impl Serialize for ChunkRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        (&self.hash, self.len).serialize(s)
    }
}

impl<'de> Deserialize<'de> for ChunkRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (hash, len) = <(String, u64)>::deserialize(d)?;
        Ok(ChunkRef { hash, len })
    }
}

/// The content of one file revision: its chunks in order.
///
/// `hash` identifies the manifest and is derived from the chunk list only, so a server can
/// verify it without reading any chunk data. `file_hash` is the BLAKE3 of the whole file and is
/// checked by clients after they rebuild the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub hash: String,
    pub size: u64,
    pub file_hash: String,
    pub chunks: Vec<ChunkRef>,
}

impl Manifest {
    pub fn new(chunks: Vec<ChunkRef>, size: u64, file_hash: String) -> Self {
        let hash = Self::compute_hash(&chunks);
        Manifest { hash, size, file_hash, chunks }
    }

    pub fn compute_hash(chunks: &[ChunkRef]) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"mp-manifest-v1");
        for c in chunks {
            match hex::decode(&c.hash) {
                Ok(raw) => h.update(&raw),
                Err(_) => h.update(c.hash.as_bytes()),
            };
            h.update(&c.len.to_le_bytes());
        }
        h.finalize().to_hex().to_string()
    }

    /// True when `hash` matches the chunk list and `size` matches the chunk lengths.
    pub fn verify(&self) -> bool {
        self.hash == Self::compute_hash(&self.chunks)
            && self.size == self.chunks.iter().map(|c| c.len).sum::<u64>()
            && self.chunks.iter().all(|c| is_hash(&c.hash))
    }
}

/// A lowercase 64-character hex string.
pub fn is_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
