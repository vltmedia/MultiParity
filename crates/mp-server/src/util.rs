use rand::RngCore;

/// A random URL-safe token (32 bytes of entropy).
pub fn new_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

/// Tokens are stored hashed so a database leak does not hand out sessions.
pub fn hash_token(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// A URL slug from a display name: lowercase ASCII letters, digits and dashes.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "project".into() } else { out }
}

/// Basic email shape check; real verification happens when the invite is accepted.
pub fn looks_like_email(s: &str) -> bool {
    let s = s.trim();
    s.len() <= 254 && s.contains('@') && !s.starts_with('@') && !s.ends_with('@') && !s.contains(' ')
}

#[cfg(test)]
mod tests {
    #[test]
    fn slugs() {
        assert_eq!(super::slugify("Cinder Works!"), "cinder-works");
        assert_eq!(super::slugify("  "), "project");
    }
}
