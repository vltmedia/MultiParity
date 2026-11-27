//! MultiParity client library, shared by the `mp` command line, the background daemon and the
//! desktop app.
//!
//! - [`accounts`]: the accounts this computer is signed in to (several at once).
//! - [`api`]: a typed HTTP client for one server and account.
//! - [`workspace`]: a project connected to a folder on this computer, and its local state.
//! - [`scan`]: finding new, edited and deleted files, honouring the ignore list.
//! - [`ops`]: connect, get latest, publish, drafts, branches, selective sync.
//! - [`checkpoints`]: private save points, on this computer or on the server.
//! - [`progress`]: how far along get latest, publish and the like are, for progress bars.

pub mod accounts;
pub mod api;
pub mod checkpoints;
pub mod ops;
pub mod progress;
pub mod scan;
pub mod types;
pub mod workspace;

pub use accounts::{Account, Accounts};
pub use api::Api;
pub use reqwest::Method as HttpMethod;
pub use workspace::Workspace;
