use std::io::Read;

use anyhow::{Context, Result};
use fastcdc::v2020::StreamCDC;

use crate::manifest::{ChunkRef, Manifest};

/// FastCDC sizes. Defaults: 256 KiB minimum, 1 MiB average, 4 MiB maximum. Small files are a
/// single chunk; identical regions across revisions of large files dedupe.
#[derive(Debug, Clone, Copy)]
pub struct ChunkParams {
    pub min: u32,
    pub avg: u32,
    pub max: u32,
}

impl Default for ChunkParams {
    fn default() -> Self {
        Self { min: 256 * 1024, avg: 1024 * 1024, max: 4 * 1024 * 1024 }
    }
}

/// Lowercase hex BLAKE3 of some bytes.
pub fn hash_bytes(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// zstd at a level that is fast to write and still saves a lot on uncompressed art formats.
pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::bulk::compress(data, 3).context("compressing chunk")
}

/// Decompress a chunk. `max` bounds the output so a hostile payload cannot exhaust memory.
pub fn decompress(data: &[u8], max: usize) -> Result<Vec<u8>> {
    zstd::bulk::decompress(data, max).context("decompressing chunk")
}

/// One chunk produced while reading a file.
pub struct Chunk {
    pub hash: String,
    pub data: Vec<u8>,
}

/// Split a stream into chunks, calling `on_chunk` for each, and return the file's manifest.
/// The whole file is never held in memory, only one chunk at a time.
pub fn chunk_reader<R: Read>(
    reader: R,
    params: ChunkParams,
    mut on_chunk: impl FnMut(Chunk) -> Result<()>,
) -> Result<Manifest> {
    let mut file_hasher = blake3::Hasher::new();
    let mut chunks = Vec::new();
    let mut size: u64 = 0;
    let mut any = false;
    for item in StreamCDC::new(reader, params.min, params.avg, params.max) {
        let c = item.context("reading file for chunking")?;
        any = true;
        file_hasher.update(&c.data);
        size += c.length as u64;
        let hash = hash_bytes(&c.data);
        chunks.push(ChunkRef { hash: hash.clone(), len: c.length as u64 });
        on_chunk(Chunk { hash, data: c.data })?;
    }
    if !any {
        // Empty file: no chunks at all.
        file_hasher.update(&[]);
    }
    Ok(Manifest::new(chunks, size, file_hasher.finalize().to_hex().to_string()))
}

/// Chunk an in-memory buffer (used by the server when it writes a merged text file).
pub fn chunk_bytes(data: &[u8], params: ChunkParams) -> Result<(Manifest, Vec<Chunk>)> {
    let mut out = Vec::new();
    let m = chunk_reader(data, params, |c| {
        out.push(c);
        Ok(())
    })?;
    Ok((m, out))
}

/// Hash a whole file without chunking (fast status checks).
pub fn hash_reader<R: Read>(mut reader: R) -> Result<(String, u64)> {
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut size = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        hasher.update(&buf[..n]);
    }
    Ok((hasher.finalize().to_hex().to_string(), size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng};

    fn random(len: usize, seed: u64) -> Vec<u8> {
        let mut r = rand::rngs::StdRng::seed_from_u64(seed);
        let mut v = vec![0u8; len];
        r.fill_bytes(&mut v);
        v
    }

    #[test]
    fn small_file_is_one_chunk() {
        let (m, chunks) = chunk_bytes(b"hello", ChunkParams::default()).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(m.size, 5);
        assert_eq!(m.file_hash, hash_bytes(b"hello"));
        assert!(m.verify());
    }

    #[test]
    fn empty_file_has_no_chunks() {
        let (m, chunks) = chunk_bytes(b"", ChunkParams::default()).unwrap();
        assert!(chunks.is_empty());
        assert_eq!(m.size, 0);
        assert_eq!(m.file_hash, hash_bytes(b""));
    }

    #[test]
    fn edit_in_the_middle_reuses_most_chunks() {
        let a = random(12 * 1024 * 1024, 7);
        let mut b = a.clone();
        b[6 * 1024 * 1024] ^= 0xff;
        let (ma, _) = chunk_bytes(&a, ChunkParams::default()).unwrap();
        let (mb, _) = chunk_bytes(&b, ChunkParams::default()).unwrap();
        let shared = mb.chunks.iter().filter(|c| ma.chunks.iter().any(|d| d.hash == c.hash)).count();
        assert!(shared + 2 >= mb.chunks.len(), "shared {shared} of {}", mb.chunks.len());
        assert_ne!(ma.hash, mb.hash);
    }

    #[test]
    fn compress_round_trip() {
        let a = random(100_000, 1);
        let c = compress(&a).unwrap();
        assert_eq!(decompress(&c, a.len()).unwrap(), a);
    }
}
