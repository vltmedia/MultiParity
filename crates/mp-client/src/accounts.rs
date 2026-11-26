//! Accounts this computer is signed in to. Several can be signed in at once, on one server or
//! many; each connected project names the account it uses.
//!
//! Stored in `<config dir>/multiparity/accounts.json` with owner-only permissions on Unix.
//! TODO: move tokens into the OS keychain (spec: Accounts and organizations).

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub server: String,
    pub email: String,
    pub name: String,
    pub token: String,
    #[serde(default)]
    pub label: Option<String>,
}

impl Account {
    /// "Justin, work account" style label for menus.
    pub fn display(&self) -> String {
        match &self.label {
            Some(l) => format!("{} ({l})", self.email),
            None => format!("{} on {}", self.email, host_of(&self.server)),
        }
    }
}

pub fn host_of(server: &str) -> String {
    server.trim_start_matches("https://").trim_start_matches("http://").trim_end_matches('/').to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Accounts {
    pub accounts: Vec<Account>,
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("MP_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("multiparity")
}

impl Accounts {
    fn path() -> PathBuf {
        config_dir().join("accounts.json")
    }

    pub fn load() -> Result<Self> {
        let p = Self::path();
        if !p.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        serde_json::from_str(&text).with_context(|| format!("{} is not valid", p.display()))
    }

    pub fn save(&self) -> Result<()> {
        let p = Self::path();
        std::fs::create_dir_all(p.parent().unwrap())?;
        crate::workspace::write_json_atomic(&p, self)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Add or replace (same server and email) an account; returns its id.
    pub fn upsert(&mut self, mut a: Account) -> String {
        if let Some(existing) = self.accounts.iter_mut().find(|x| x.server == a.server && x.email == a.email) {
            a.id = existing.id.clone();
            if a.label.is_none() {
                a.label = existing.label.clone();
            }
            *existing = a.clone();
            return a.id;
        }
        self.accounts.push(a.clone());
        a.id
    }

    /// Find by id, email, label, or server host. Errors when ambiguous.
    pub fn find(&self, key: &str) -> Result<&Account> {
        let k = key.to_lowercase();
        let hits: Vec<&Account> = self
            .accounts
            .iter()
            .filter(|a| a.id == key || a.email.to_lowercase() == k || a.label.as_deref().map(|l| l.to_lowercase()) == Some(k.clone()) || host_of(&a.server) == k)
            .collect();
        match hits.len() {
            1 => Ok(hits[0]),
            0 => anyhow::bail!("No signed-in account matches {key}. Run `mp accounts` to see them, or `mp login` to add one."),
            _ => anyhow::bail!("More than one account matches {key}. Use the account id from `mp accounts`."),
        }
    }

    pub fn by_id(&self, id: &str) -> Result<&Account> {
        self.accounts
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| anyhow::anyhow!("This folder's project uses an account that is no longer signed in on this computer. Run `mp login` for it, then `mp project use-account`."))
    }

    /// The only account, or the one matching `key`.
    pub fn pick(&self, key: Option<&str>) -> Result<&Account> {
        match key {
            Some(k) => self.find(k),
            None => match self.accounts.len() {
                0 => anyhow::bail!("You are not signed in. Run `mp login <server>` first."),
                1 => Ok(&self.accounts[0]),
                _ => anyhow::bail!("You are signed in to several accounts. Choose one with --account."),
            },
        }
    }
}
