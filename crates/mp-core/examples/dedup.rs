//! Measures how much new data each version of a file adds, with the same chunking and
//! compression MultiParity uses for publishing and checkpoints.
//!
//!   cargo run --release -p mp-core --example dedup -- v1.blend v2.blend v3.blend
//!
//! For each file, in order: its size, and the bytes in parts not seen in any earlier file
//! (raw, and compressed as they would be stored or uploaded).

use std::collections::HashSet;
use std::io::BufReader;

use mp_core::chunk::{chunk_reader, ChunkParams};

fn mb(b: u64) -> String {
    format!("{:>9.1}", b as f64 / 1_048_576.0)
}

fn main() -> anyhow::Result<()> {
    let files: Vec<String> = std::env::args().skip(1).collect();
    anyhow::ensure!(!files.is_empty(), "name one or more files, oldest first");
    let mut seen: HashSet<String> = HashSet::new();
    println!("{:<14} {:>9} {:>7} {:>9} {:>9} {:>9} {:>7}", "file", "size MB", "parts", "new parts", "new MB", "stored MB", "new %");
    for path in &files {
        let f = std::fs::File::open(path)?;
        let (mut parts, mut new_parts, mut new_raw, mut new_stored) = (0u64, 0u64, 0u64, 0u64);
        let manifest = chunk_reader(BufReader::with_capacity(1 << 20, f), ChunkParams::default(), |c| {
            parts += 1;
            if seen.insert(c.hash.clone()) {
                new_parts += 1;
                new_raw += c.data.len() as u64;
                new_stored += mp_core::compress(&c.data)?.len() as u64;
            }
            Ok(())
        })?;
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        println!(
            "{:<14} {} {:>7} {:>9} {} {} {:>6.1}%",
            name,
            mb(manifest.size),
            parts,
            new_parts,
            mb(new_raw),
            mb(new_stored),
            100.0 * new_raw as f64 / manifest.size.max(1) as f64
        );
    }
    Ok(())
}
