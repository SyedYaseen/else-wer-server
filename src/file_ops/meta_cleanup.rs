use crate::models::fs_models::FileScanCache;
use lazy_static::lazy_static;
use regex::Regex;
use strsim::levenshtein;

lazy_static! {
    static ref DRAMATIZED: Regex = Regex::new(r"(?i)\s*[\(\[]\s*(dramatized|dramatised|graphic audio)\s*[\)\]]").unwrap();
    static ref REMOVE_TERMS: Regex = Regex::new(r"(?i)\s*[\(\[]\s*(abridged|unabridged|audible|special edition|dramatized|dramatised|graphic audio)\s*[\)\]]").unwrap();
    static ref BRACKETS: Regex = Regex::new(r"(?i)\([^)]*\)|\[[^\]]*\]|\{[^}]*\}").unwrap();
    static ref BRACKET_CONTENT: Regex = Regex::new(r"(?i)\(([^)]*)\)|\[([^\]]*)\]|\{([^}]*)\}").unwrap();
    static ref TRAILING_SEPARATORS: Regex = Regex::new(r"(?i)\s*(-|–|—|::|by)\s*$").unwrap();

    static ref MULTIPLE_SPACES: Regex = Regex::new(r"\s{2,}").unwrap();
    static ref BOOK_ORDER_TOKENS: Regex = Regex::new(r"(?i)\b(?:book|vol|volume|part|disc|)?\s*(-?\d+(?:-\d+)?)\b").unwrap();
    static ref ORDER_TOKENS: Regex = Regex::new(r"(?i)\b(?:track|episode|ep|part|chapter)?\s*(-?\d+(?:-\d+)?)\b").unwrap();
        // static ref COMMON_TERMS: Regex = Regex::new(
    //     r"(?i)\b(unabridged|abridged|audible|original|special edition|read by|narrated by|a novel|track)\b"
    // ).unwrap();
    // static ref REMOVE_TERMS: Regex = Regex::new(
    //     r"(?i)\b(unabridged|abridged|audible|original|special edition|dramatized|read by|narrated by|a novel|track)\b"
    // ).unwrap();
}

fn is_dramatized(text: &String) -> bool {
    fuzzy_contain(&text.to_lowercase(), "graphic audio", 2)
        || fuzzy_contain(&text.to_lowercase(), "dramatized", 2)
}
/// Clean metadata and extract bracket contents
pub fn clean_metadata(text: &String) -> (String, Vec<String>) {
    let mut bracket_info = Vec::new();

    let mut result = REMOVE_TERMS.replace(text, "").to_string();
    for caps in BRACKET_CONTENT.captures_iter(&result) {
        for i in 1..=3 {
            if let Some(m) = caps.get(i) {
                bracket_info.push(m.as_str().trim().to_string());
            }
        }
    }

    // Remove brackets + common terms + trailing separators
    result = BRACKETS.replace_all(text, "").to_string();
    result = TRAILING_SEPARATORS.replace_all(&result, "").to_string();
    // result = REMOVE_TERMS.replace_all(&result, "").to_string();

    // Clean whitespace
    result = result.trim().to_string();
    result = MULTIPLE_SPACES.replace_all(&result, " ").to_string();

    (result, bracket_info)
}

/// Extract order number if present (Book 1, Part 2, etc.)
fn capture_order(text: &str) -> Option<i64> {
    for caps in BOOK_ORDER_TOKENS.captures_iter(text) {
        println!("{:#?}", caps);

        // if let Some(num) = caps.get(2) {
        //     if let Ok(n) = num.as_str().parse::<i32>() {
        //         return Some(n as i64);
        //     }
        // }
    }
    None
}

/// Sort titles by order token if present, otherwise alphabetically
// pub fn sort_by_order(titles: &mut [String]) {
//     titles.sort_by(|a, b| {
//         let order_a = extract_order(a);
//         let order_b = extract_order(b);

//         match (order_a, order_b) {
//             (Some(na), Some(nb)) => na.cmp(&nb),
//             (Some(_), None) => Ordering::Less,
//             (None, Some(_)) => Ordering::Greater,
//             (None, None) => a.cmp(b),
//         }
//     });
// }

fn assign_title_if_empty(metadata: &mut FileScanCache) {
    if metadata.title.is_none() || metadata.title == Some("".to_string()) {
        let fname = metadata.file_name.clone();
        if let Some(n) = fname.split(".").nth(0) {
            if !n.parse::<i64>().is_ok() {
                metadata.title = Some(n.to_string());
            }
        }
    }

    if metadata.title.is_none()
        || metadata.title == Some("".to_string()) && metadata.series.is_some()
    {
        metadata.title = metadata.series.clone();
    }
}

fn assign_track_number(metadata: &mut FileScanCache) {
    if metadata.track_number.is_some() {
        return;
    }

    let mut track_num = 0;
    let sources: Vec<Option<&String>> = vec![
        metadata.title.as_ref(),
        Some(&metadata.file_name),
        metadata.series.as_ref(),
    ];

    for src in sources {
        if metadata.track_number.is_some() {
            break;
        }

        if let Some(meta) = src {
            let order = capture_order(meta.as_str());
            metadata.track_number = order;
        }
    }
}

fn series_cleanup(metadata: &mut FileScanCache) {
    if let Some(series) = &metadata.series {
        if is_dramatized(series) {
            metadata.dramatized = true;
        }
        let (clean_series, extracted_info) = clean_metadata(series);
        metadata.clean_series = Some(clean_series);

        capture_order(&series);

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
}

fn author_cleanup(metadata: &mut FileScanCache) {
    if let Some(author) = &metadata.author {
        if is_dramatized(author) {
            metadata.dramatized = true;
        }
    }
}

pub fn meta_cleanup(metadata: &mut FileScanCache) {
    series_cleanup(metadata);
    author_cleanup(metadata);

    // assign_track_number(metadata);

    println!("");
    // assign_title_if_empty(metadata);
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
