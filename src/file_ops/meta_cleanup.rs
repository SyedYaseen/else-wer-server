use crate::{api::api_error::ApiError, models::meta_scan::FileScanCache};
use lazy_static::lazy_static;
use regex::Regex;
use sqlx::SqlitePool;
use strsim::levenshtein;

lazy_static! {
    static ref REMOVE_TERMS: Regex = Regex::new(r"(?i)\s*[\(\[]\s*(abridged|unabridged|audible|special edition)\s*[\)\]]").unwrap();
    static ref BRACKETS: Regex = Regex::new(r"(?i)\([^)]*\)|\[[^\]]*\]|\{[^}]*\}").unwrap();
    static ref BRACKET_CONTENT: Regex = Regex::new(r"(?i)\(([^)]*)\)|\[([^\]]*)\]|\{([^}]*)\}").unwrap();
    static ref TRAILING_SEPARATORS: Regex = Regex::new(r"(?i)\s*(-|–|—|::|by)\s*$").unwrap();
    static ref MULTIPLE_SPACES: Regex = Regex::new(r"\s{2,}").unwrap();
    static ref DISC_ORDER_TOKENS: Regex = Regex::new(r"(?i)\b(?:vol|volume|part|disc)?\s*(-?\d+(?:-\d+)?)\b").unwrap();

    static ref PART_REMOVAL: Regex = Regex::new(r"(?i)\b(?:vol|volume|part|disc)[\s._-]*\d+\b|\bdisc\b").unwrap();

    // Trailing paren/bracket may be unclosed in real tags: "j.r.r. tolkien (narr. christopher lee"
    static ref NARRATOR: Regex = Regex::new(r"(?i)[\(\[,]?\s*(?:narr(?:ated)?\.?\s*(?:by)?|read by)\s+(.+?)[\)\]]?\s*$").unwrap();

    // static ref BOOK_ORDER_TOKENS: Regex = Regex::new(r"(?i)\b(?:book|part)?\s*(-?\d+(?:-\d+)?)\b").unwrap();
    // static ref FILE_ORDER_TOKENS: Regex = Regex::new(r"(?i)\b(?:track|episode|ep|part|chapter)?\s*(-?\d+(?:-\d+)?)\b").unwrap();
}

fn is_dramatized(text: &String) -> bool {
    fuzzy_contain(&text.to_lowercase(), "graphic audio", 2)
        || fuzzy_contain(&text.to_lowercase(), "dramatized", 2)
}
/// Clean metadata and extract bracket contents
pub fn clean_metadata(text: &String) -> (String, Vec<String>) {
    let mut bracket_info = Vec::new();

    let mut result = REMOVE_TERMS.replace(text, "").to_string();

    result = PART_REMOVAL.replace(&result, "").to_string();

    for caps in BRACKET_CONTENT.captures_iter(&result) {
        for i in 1..=3 {
            if let Some(m) = caps.get(i) {
                bracket_info.push(m.as_str().trim().to_string());
            }
        }
    }

    // Remove brackets + common terms + trailing separators
    result = BRACKETS.replace_all(&result, "").to_string();
    result = TRAILING_SEPARATORS.replace_all(&result, "").to_string();
    // result = REMOVE_TERMS.replace_all(&result, "").to_string();

    // Clean whitespace
    result = result.trim().to_string();
    result = MULTIPLE_SPACES.replace_all(&result, " ").to_string();

    (result, bracket_info)
}

/// Extract order number if present (Book 1, Part 2, etc.)
fn capture_disc_order(text: &str) -> Option<i64> {
    for caps in DISC_ORDER_TOKENS.captures_iter(text) {
        if let Some(num) = caps.get(1) {
            if let Ok(n) = num.as_str().parse::<i32>() {
                return Some(n as i64);
            }
        }
    }
    None
}

fn series_cleanup(metadata: &mut FileScanCache) {
    match &metadata.series {
        Some(series) => {
            if is_dramatized(series) {
                metadata.dramatized = true;
            }

            metadata.disc_number = capture_disc_order(&series);

            let (clean_series, extracted_info) = clean_metadata(series);
            metadata.clean_series = Some(clean_series);

            // let order_cleared_series = match &metadata.clean_series {
            //     Some(val) => Some(BOOK_ORDER_TOKENS.replace_all(val, "").trim().to_string()),
            //     None => None,
            // };

            // metadata.clean_series = order_cleared_series;

            if extracted_info.iter().len() > 0 {
                let joined_extract = extracted_info.join(",");

                match &metadata.extracts {
                    Some(val) => {
                        metadata.extracts = Some(format!("{} | {}", val, joined_extract));
                    }
                    None => {
                        metadata.extracts = Some(joined_extract);
                    }
                }
            }
        }
        None => {
            if metadata.title.is_some() {
                metadata.clean_series = metadata.clean_title.clone();
            }
        }
    }
}

fn author_cleanup(metadata: &mut FileScanCache) {
    let mut clean_author = String::new();
    if let Some(author) = &metadata.author {
        if is_dramatized(author) {
            metadata.dramatized = true;
        }

        let author = match NARRATOR.captures(author) {
            Some(caps) => {
                if metadata.narrated_by.is_none() {
                    metadata.narrated_by = caps.get(1).map(|m| m.as_str().trim().to_string());
                }
                let m = caps.get(0).unwrap();
                author[..m.start()].to_string()
            }
            None => author.clone(),
        };

        clean_author = clean_metadata(&author).0;
    }
    if !clean_author.is_empty() {
        metadata.author = Some(clean_author);
    }
}

/// Diacritic-folded lowercase key for grouping comparisons only (Húrin == hurin).
/// Stored display values keep their diacritics.
pub fn fold_key(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/*
* If no title exists, apply cleaned series value else cleaned filename value
* If title exists, clean and add it as clean title
*/
fn title_cleanup(metadata: &mut FileScanCache) {
    let clean_title: Option<String> = match &metadata.title {
        None => {
            let mut title: Option<String> = {
                if let Some(series) = &metadata.series {
                    Some(series.clone())
                } else {
                    let fname = metadata.file_name.clone();
                    if let Some(n) = fname.split('.').next() {
                        if n.parse::<i64>().is_err() {
                            Some(n.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            };

            if let Some(raw_title) = &title {
                title = Some(clean_metadata(raw_title).0);
                title
            } else {
                None
            }
        }
        Some(title) => Some(clean_metadata(title).0),
    };

    metadata.clean_title = clean_title;
}

pub fn meta_cleanup(metadata: &mut FileScanCache) {
    title_cleanup(metadata);
    author_cleanup(metadata);
    series_cleanup(metadata);

    // assign_track_number(metadata);

    println!("");
    // println!(
    //     "{} {}",
    //     metadata.file_name,
    //     metadata.author.as_deref().unwrap_or_default()
    // );
    // if let Some(title) = metadata.title.clone() {
    //     let (title, _parts): (String, Vec<String>) = clean_metadata(&title);
    //     metadata.title = Some(title);
    // }
}

fn fuzzy_contain(text: &String, phrase: &str, threshold: usize) -> bool {
    let clean_text: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();

    let words: Vec<&str> = clean_text.split_whitespace().collect();
    let phrase_words: Vec<&str> = phrase.split_whitespace().collect();

    words.windows(phrase_words.len()).any(|window| {
        window
            .iter()
            .zip(&phrase_words)
            .all(|(w, p)| levenshtein(w, p) <= threshold)
    })
}

pub async fn grouped_meta_cleanup(db: &SqlitePool) -> Result<(), ApiError> {
    //let grouped_data = group_meta_fetch(db).await?;
    // grouped_data.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fsc_with_author(author: &str) -> FileScanCache {
        let mut m = FileScanCache::new("/x/a.mp3".into(), "a.mp3".into(), "/x".into());
        m.author = Some(author.to_string());
        m
    }

    #[test]
    fn narrator_strip_unclosed_paren() {
        let mut m = fsc_with_author("j.r.r. tolkien (narr. christopher lee");
        author_cleanup(&mut m);
        assert_eq!(m.author.as_deref(), Some("j.r.r. tolkien"));
        assert_eq!(m.narrated_by.as_deref(), Some("christopher lee"));
    }

    #[test]
    fn narrator_strip_closed_paren() {
        let mut m = fsc_with_author("brandon sanderson (narrated by michael kramer)");
        author_cleanup(&mut m);
        assert_eq!(m.author.as_deref(), Some("brandon sanderson"));
        assert_eq!(m.narrated_by.as_deref(), Some("michael kramer"));
    }

    #[test]
    fn narrator_strip_read_by_comma() {
        let mut m = fsc_with_author("ursula k. le guin, read by rob inglis");
        author_cleanup(&mut m);
        assert_eq!(m.author.as_deref(), Some("ursula k. le guin"));
        assert_eq!(m.narrated_by.as_deref(), Some("rob inglis"));
    }

    #[test]
    fn author_without_narrator_unchanged() {
        let mut m = fsc_with_author("j.r.r. tolkien");
        author_cleanup(&mut m);
        assert_eq!(m.author.as_deref(), Some("j.r.r. tolkien"));
        assert_eq!(m.narrated_by, None);
    }

    #[test]
    fn existing_narrated_by_not_overwritten() {
        let mut m = fsc_with_author("tolkien narrated by someone else");
        m.narrated_by = Some("from tag".to_string());
        author_cleanup(&mut m);
        assert_eq!(m.narrated_by.as_deref(), Some("from tag"));
        assert_eq!(m.author.as_deref(), Some("tolkien"));
    }

    #[test]
    fn fold_key_diacritics() {
        assert_eq!(fold_key("Húrin"), "hurin");
        assert_eq!(fold_key("Türin"), "turin");
        assert_eq!(fold_key("plain"), "plain");
    }
}

// fn assign_track_number(metadata: &mut FileScanCache) {
//     if metadata.track_number.is_some() {
//         return;
//     }

//     let mut track_num = 0;
//     let sources: Vec<Option<&String>> = vec![
//         metadata.title.as_ref(),
//         Some(&metadata.file_name),
//         metadata.series.as_ref(),
//     ];

//     for src in sources {
//         if metadata.track_number.is_some() {
//             break;
//         }

//         if let Some(meta) = src {
//             let order = capture_order(meta.as_str());
//             metadata.track_number = order;
//         }
//     }
// }
