use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DEFAULT_MAX_CHARS: usize = 4000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Chunk {
    /// Where in the source: "p. 12", a heading path like "Horns > Throat", or "§1".
    pub locator: String,
    pub text: String,
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with([' ', '\t']) {
        Some((hashes, line[hashes..].trim()))
    } else {
        None
    }
}

/// Splits extracted text into searchable chunks: one per page for `pdftotext` output (form feeds),
/// one per heading for Markdown, otherwise a single chunk; long chunks are split further.
pub fn chunk(text: &str, max_chars: usize) -> Vec<Chunk> {
    let mut sections: Vec<(String, String)> = vec![];
    if text.contains('\x0c') {
        for (i, page) in text.split('\x0c').enumerate() {
            sections.push((format!("p. {}", i + 1), page.to_string()));
        }
    } else if text.lines().any(|l| heading(l).is_some()) {
        let mut path: Vec<(usize, String)> = vec![];
        let mut current = (String::from("§1"), String::new());
        for line in text.lines() {
            if let Some((level, title)) = heading(line) {
                sections.push(std::mem::take(&mut current));
                path.retain(|(l, _)| *l < level);
                path.push((level, title.to_string()));
                let locator = path.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" > ");
                current = (locator, String::new());
            } else {
                current.1.push_str(line);
                current.1.push('\n');
            }
        }
        sections.push(current);
    } else {
        sections.push(("§1".into(), text.to_string()));
    }
    sections
        .into_iter()
        .filter(|(_, t)| !t.trim().is_empty())
        .flat_map(|(loc, t)| split_long(&loc, t.trim(), max_chars))
        .collect()
}

fn split_long(locator: &str, text: &str, max_chars: usize) -> Vec<Chunk> {
    if text.chars().count() <= max_chars {
        return vec![Chunk { locator: locator.into(), text: text.into() }];
    }
    let mut pieces = vec![];
    let mut rest = text;
    while !rest.is_empty() {
        if rest.chars().count() <= max_chars {
            pieces.push(rest.to_string());
            break;
        }
        let limit = rest.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(rest.len());
        let window = &rest[..limit];
        let cut = window.rfind("\n\n").or_else(|| window.rfind('\n')).filter(|&i| i > 0).unwrap_or(limit);
        pieces.push(rest[..cut].trim().to_string());
        rest = rest[cut..].trim_start();
    }
    pieces
        .into_iter()
        .filter(|p| !p.is_empty())
        .enumerate()
        .map(|(i, text)| Chunk { locator: format!("{locator} (part {})", i + 1), text })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locs(c: &[Chunk]) -> Vec<&str> {
        c.iter().map(|c| c.locator.as_str()).collect()
    }

    #[test]
    fn pdftotext_pages_get_page_locators() {
        let c = chunk("a\x0cb\x0c\x0cd", 4000);
        assert_eq!(locs(&c), ["p. 1", "p. 2", "p. 4"]);
        assert_eq!(c[2].text, "d");
    }

    #[test]
    fn markdown_heading_paths() {
        let c = chunk("intro text\n# A\nalpha\n## B\nbeta\n# C\ngamma", 4000);
        assert_eq!(locs(&c), ["§1", "A", "A > B", "C"]);
        assert!(c[2].text.contains("beta"));
    }

    #[test]
    fn plain_text_single_chunk() {
        let c = chunk("just some words\nand more", 4000);
        assert_eq!(locs(&c), ["§1"]);
        assert!(chunk("   \n ", 4000).is_empty());
    }

    #[test]
    fn long_chunk_split_on_blank_line_with_part_suffix() {
        let text = format!("{}\n\n{}", "a".repeat(30), "b".repeat(30));
        let c = chunk(&text, 40);
        assert_eq!(locs(&c), ["§1 (part 1)", "§1 (part 2)"]);
        assert_eq!(c[0].text, "a".repeat(30));
        let hard = chunk(&"x".repeat(100), 40);
        assert_eq!(hard.len(), 3);
        assert!(hard.iter().all(|c| c.text.chars().count() <= 40));
        let multibyte = chunk(&"é".repeat(100), 40);
        assert!(multibyte.iter().all(|c| c.text.chars().count() <= 40));
    }
}
