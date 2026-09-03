//! Reading a plugin's file-backed sources (spec §4.5).

/// Split a `---`-delimited frontmatter block from the body that follows.
///
/// Returns `None` when the text does not open with a delimiter, or opens one it
/// never closes. Both cases mean "no frontmatter here", and neither may be
/// reported as an empty frontmatter: a file with no frontmatter contributes no
/// Resident source, which is different from contributing zero bytes.
///
/// Line endings are matched tolerantly. This repository is developed on Windows
/// and does not pin these files in `.gitattributes`, so a checked-out `SKILL.md`
/// can carry CRLF; a splitter keyed on `"---\n"` alone would find no frontmatter
/// and drop every skill out of the Resident tier without failing anything.
pub fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let after_open = strip_delimiter_line(text)?;

    let mut offset = 0;
    while offset < after_open.len() {
        let rest = &after_open[offset..];
        if let Some(body) = strip_delimiter_line(rest) {
            return Some((&after_open[..offset], body));
        }
        // Advance to the start of the next line.
        match rest.find('\n') {
            Some(newline) => offset += newline + 1,
            None => break,
        }
    }
    None
}

/// If `text` begins with a `---` line, return what follows it.
fn strip_delimiter_line(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---")?;
    // Accept CRLF, LF, and a delimiter that ends the file.
    if let Some(rest) = rest.strip_prefix("\r\n") {
        return Some(rest);
    }
    if let Some(rest) = rest.strip_prefix('\n') {
        return Some(rest);
    }
    rest.is_empty().then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_the_closing_delimiter() {
        let text = "---\nname: doctor\ndescription: d\n---\n\n# Body\ntext\n";

        let (front, body) = split_frontmatter(text).expect("has frontmatter");

        assert_eq!(front, "name: doctor\ndescription: d\n");
        assert_eq!(body, "\n# Body\ntext\n");
    }

    #[test]
    fn a_file_without_frontmatter_has_none() {
        // A plain Markdown file is not an error — it simply contributes no
        // frontmatter source. Returning an empty frontmatter instead would
        // report a measurement of zero for something never measured.
        assert!(split_frontmatter("# Just a heading\n").is_none());
    }

    #[test]
    fn an_unterminated_frontmatter_block_is_not_frontmatter() {
        // Treating the whole file as frontmatter would move a body's bytes into
        // the Resident tier, overstating what the host holds on every request.
        assert!(split_frontmatter("---\nname: x\nno closing delimiter\n").is_none());
    }

    #[test]
    fn a_delimiter_inside_the_body_does_not_reopen_the_block() {
        let text = "---\nname: x\n---\nbody\n---\nmore body\n";

        let (front, body) = split_frontmatter(text).expect("has frontmatter");

        assert_eq!(front, "name: x\n");
        assert_eq!(body, "body\n---\nmore body\n");
    }

    #[test]
    fn crlf_line_endings_split_the_same_way() {
        // This repository is developed on Windows and `.gitattributes` is not
        // pinning these files, so a checked-out SKILL.md can carry CRLF. A
        // splitter that only recognised "---\n" would find no frontmatter and
        // silently drop every skill from the Resident tier.
        let text = "---\r\nname: x\r\n---\r\nbody\r\n";

        let (front, body) = split_frontmatter(text).expect("has frontmatter");

        assert_eq!(front, "name: x\r\n");
        assert_eq!(body, "body\r\n");
    }
}
