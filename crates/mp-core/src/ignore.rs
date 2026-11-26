//! Suggestions for the right-click "Ignore" menu. Given one file's project-relative path, offer
//! patterns from narrowest to widest. Counting what each would match is the caller's job.

use globset::{GlobBuilder, GlobMatcher};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub pattern: String,
}

const JUNK_WORDS: &[&str] = &["autosave", "backup", "bak", "tmp", "temp", "copy", "old", "cache"];

/// Patterns for one path, narrowest first, without duplicates.
pub fn suggest(path: &str) -> Vec<Suggestion> {
    let path = path.trim_start_matches('/');
    let (dir, name) = match path.rsplit_once('/') {
        Some((d, n)) => (d, n),
        None => ("", path),
    };
    let ext = name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty());
    let mut out: Vec<Suggestion> = Vec::new();
    let mut push = |label: String, pattern: String| {
        if !out.iter().any(|s| s.pattern == pattern) {
            out.push(Suggestion { label, pattern });
        }
    };

    push("Only this file".into(), format!("/{path}"));

    let like = wildcard_numbers(name);
    if like != name {
        push("Files named like this".into(), like);
    }

    let lower = name.to_ascii_lowercase();
    for w in JUNK_WORDS {
        if lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|part| part == *w)
            || (w.len() >= 6 && lower.contains(w))
        {
            push(format!("Anything with {w} in the name"), format!("*{w}*"));
            break;
        }
    }

    if let Some(ext) = ext {
        if !dir.is_empty() {
            push("This kind of file, in this folder".into(), format!("/{dir}/*.{ext}"));
        }
        push("This kind of file, everywhere".into(), format!("*.{ext}"));
    }
    if !dir.is_empty() {
        push("Everything in this folder".into(), format!("/{dir}/"));
    }
    out
}

/// Replace runs of digits (and digit-led date or version fragments) with `*`, collapsing
/// neighbouring wildcards, so `shot_0142_v03.blend1` becomes `shot_*_v*.blend1`.
pub fn wildcard_numbers(name: &str) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, Some(e)),
        _ => (name, None),
    };
    let mut out = wildcard_stem(stem);
    if let Some(e) = ext {
        out.push('.');
        out.push_str(e);
    }
    out
}

fn wildcard_stem(name: &str) -> String {
    let mut out = String::new();
    let mut in_digits = false;
    for c in name.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('*');
                in_digits = true;
            }
        } else {
            in_digits = false;
            out.push(c);
        }
    }
    while out.contains("*-*") || out.contains("*_*") || out.contains("**") {
        out = out.replace("*-*", "*").replace("*_*", "*").replace("**", "*");
    }
    out
}

/// Gitignore-style match of one pattern against a project-relative file path. A pattern with a
/// leading `/` is anchored to the project root; a trailing `/` matches everything inside that
/// folder; otherwise a pattern without `/` matches the file name at any depth.
pub fn pattern_matches(pattern: &str, path: &str) -> bool {
    let Some(m) = compile(pattern) else { return false };
    m.is_match(path)
}

fn compile(pattern: &str) -> Option<GlobMatcher> {
    let p = pattern.trim();
    if p.is_empty() || p.starts_with('#') || p.starts_with('!') {
        return None;
    }
    let (anchored, body) = match p.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, p),
    };
    let dir_only = body.ends_with('/');
    let body = body.trim_end_matches('/');
    let mut glob = if anchored || body.contains('/') { body.to_string() } else { format!("**/{body}") };
    if dir_only {
        glob.push_str("/**");
    }
    GlobBuilder::new(&glob).literal_separator(true).build().ok().map(|g| g.compile_matcher())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_for_an_autosave() {
        let s = suggest("Characters/Hero/hero_body_autosave_0142.blend1");
        let pats: Vec<_> = s.iter().map(|s| s.pattern.as_str()).collect();
        assert_eq!(
            pats,
            vec![
                "/Characters/Hero/hero_body_autosave_0142.blend1",
                "hero_body_autosave_*.blend1",
                "*autosave*",
                "/Characters/Hero/*.blend1",
                "*.blend1",
                "/Characters/Hero/",
            ]
        );
    }

    #[test]
    fn matching() {
        assert!(pattern_matches("*.blend1", "a/b/c.blend1"));
        assert!(pattern_matches("/Characters/Hero/", "Characters/Hero/x/y.png"));
        assert!(!pattern_matches("/Characters/Hero/", "Characters/Heroes/y.png"));
        assert!(pattern_matches("hero_*.png", "deep/hero_01.png"));
        assert!(pattern_matches("Saved/", "Saved/Logs/a.log"));
        assert!(!pattern_matches("/Saved/", "Game/Saved/a.log"));
    }

    #[test]
    fn wildcards() {
        assert_eq!(wildcard_numbers("shot_0142_v03.blend1"), "shot_*_v*.blend1");
        assert_eq!(wildcard_numbers("render_2026-11-26.exr"), "render_*.exr");
    }
}
