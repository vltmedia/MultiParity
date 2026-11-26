/// Normalise a project-relative path: forward slashes, no leading `./` or `/`, no empty parts.
/// Returns `None` for anything that could escape the project (`..`, absolute, drive letters) or
/// that names MultiParity's own folder.
pub fn normalize(path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    if p.starts_with('/') || p.contains(':') {
        return None;
    }
    let mut parts = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            other => {
                if other.chars().any(|c| c.is_control()) {
                    return None;
                }
                parts.push(other)
            }
        }
    }
    if parts.is_empty() || parts[0] == ".mp" {
        return None;
    }
    let out = parts.join("/");
    if out.len() > 1024 {
        return None;
    }
    Some(out)
}

/// True when `path` is inside folder `prefix` (or is it). An empty prefix contains everything.
pub fn within(path: &str, prefix: &str) -> bool {
    let prefix = prefix.trim_end_matches('/');
    prefix.is_empty() || path == prefix || (path.starts_with(prefix) && path.as_bytes().get(prefix.len()) == Some(&b'/'))
}

/// Selective sync: a path is on this computer when it is inside one of `include` (or include is
/// empty) and not inside any of `exclude`.
pub fn in_scope(path: &str, include: &[String], exclude: &[String]) -> bool {
    (include.is_empty() || include.iter().any(|i| within(path, i)))
        && !exclude.iter().any(|e| within(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes() {
        assert_eq!(normalize("./a\\b//c.png").as_deref(), Some("a/b/c.png"));
        assert_eq!(normalize("../x"), None);
        assert_eq!(normalize("/etc/passwd"), None);
        assert_eq!(normalize("C:/x"), None);
        assert_eq!(normalize(".mp/index.json"), None);
    }
    #[test]
    fn scope() {
        assert!(within("Art/Hero/a.png", "Art"));
        assert!(!within("Artwork/a.png", "Art"));
        let inc = vec!["Art".to_string()];
        let exc = vec!["Art/Video".to_string()];
        assert!(in_scope("Art/a.png", &inc, &exc));
        assert!(!in_scope("Art/Video/a.mov", &inc, &exc));
        assert!(!in_scope("Code/a.rs", &inc, &exc));
        assert!(in_scope("Code/a.rs", &[], &[]));
    }
}
