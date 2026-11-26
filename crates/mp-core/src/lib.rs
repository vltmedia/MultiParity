//! MultiParity core: the pieces shared by the server, the client and the daemon.
//!
//! - [`chunk`]: content-defined chunking (FastCDC), BLAKE3 hashing, zstd compression.
//! - [`manifest`]: the ordered chunk list that describes one file's content.
//! - [`kind`]: file kinds derived from extensions, and which kinds are claimed (locked).
//! - [`ignore`]: ignore-pattern suggestions for the right-click "Ignore" menu.
//! - [`paths`]: project-relative path normalisation and validation.
//! - [`merge`]: three-way text merge used when branches are updated.

pub mod chunk;
pub mod ignore;
pub mod kind;
pub mod manifest;
pub mod merge;
pub mod paths;

pub use chunk::{compress, decompress, hash_bytes, ChunkParams};
pub use manifest::{ChunkRef, Manifest};
