//! Shapes returned by the server, kept forgiving (`#[serde(default)]`) so a newer server with
//! extra fields does not break an older client.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UserRef {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OrgRef {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Me {
    pub id: String,
    pub email: String,
    pub name: String,
    pub is_server_admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProjectSummary {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub org: OrgRef,
    pub archived: bool,
    pub head_change: i64,
    pub size: i64,
    pub file_count: i64,
    pub claims: i64,
    pub my_claims: i64,
    pub branches_list: Vec<Branch>,
    /// Where checkpoints are kept for this project: "server" or "local".
    pub checkpoint_storage: String,
    /// The project's setting: "auto", "server" or "local".
    pub checkpoint_setting: String,
    /// Whether the project asks for automatic checkpoints while people work.
    pub auto_checkpoints: bool,
    /// The project's look ({light, dark, own_light, own_dark}), from the project or its
    /// organization; passed through to the desktop app.
    pub theme: Option<serde_json::Value>,
    /// Where it is filed: its client ({id, name}) and production ({id, name, main_project_id}),
    /// both optional; passed through to the desktop app.
    pub client: Option<serde_json::Value>,
    pub production: Option<serde_json::Value>,
    /// The main project of its production: listed first.
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub shares_claims: bool,
    pub protected: bool,
    pub archived: bool,
    pub head_change: i64,
    pub ahead: i64,
    pub behind: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TreeEntry {
    pub path: String,
    pub file_hash: Option<String>,
    pub manifest_hash: Option<String>,
    pub size: i64,
    pub change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TreeDelta {
    pub head: i64,
    pub entries: Vec<TreeEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Change {
    pub number: i64,
    pub branch_id: String,
    pub message: String,
    pub author: Option<UserRef>,
    pub created_at: String,
    pub kind: String,
    pub file_count: i64,
    pub size: i64,
}

/// One published version of a file: the change that added, edited or deleted it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct FileVersion {
    pub change: i64,
    /// add, edit or delete
    pub op: String,
    pub size: i64,
    pub file_hash: Option<String>,
    pub manifest_hash: Option<String>,
    pub message: String,
    pub created_at: String,
    pub branch_id: String,
    pub branch_name: String,
    pub author: Option<UserRef>,
    /// The version the branch has now.
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Claim {
    pub path: String,
    pub branch_id: String,
    pub user: Option<UserRef>,
    pub device: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEntry {
    pub path: String,
    pub op: String,
    pub prev_file_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<mp_core::Manifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MergeReport {
    pub up_to_date: bool,
    pub only_here: i64,
    pub only_parent: i64,
    pub auto_merged: Vec<String>,
    pub conflicts: Vec<ConflictItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ConflictItem {
    pub path: String,
    pub kind: String,
}
