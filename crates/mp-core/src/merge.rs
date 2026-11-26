/// Three-way merge of text. `Ok` when the two sides do not overlap, `Err` with the conflicting
/// text (with markers) otherwise. `base` is `None` when the file did not exist in the common
/// ancestor: then only identical sides merge.
pub fn merge_text(base: Option<&str>, mine: &str, theirs: &str) -> Result<String, String> {
    if mine == theirs {
        return Ok(mine.to_string());
    }
    match base {
        None => Err(String::new()),
        Some(b) => diffy::merge(b, mine, theirs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_overlapping_edits_merge() {
        let base = "a\nb\nc\nd\ne\n";
        let mine = "A\nb\nc\nd\ne\n";
        let theirs = "a\nb\nc\nd\nE\n";
        assert_eq!(merge_text(Some(base), mine, theirs).unwrap(), "A\nb\nc\nd\nE\n");
    }
    #[test]
    fn overlapping_edits_conflict() {
        assert!(merge_text(Some("a\n"), "b\n", "c\n").is_err());
    }
}
