use crate::{
    api::api_error::ApiError, db::meta_scan::group_meta_fetch, models::meta_scan::FileScanCache,
};
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
    static ref DISC_REMOVAL: Regex = Regex::new(r"(?i)\b(?:vol|volume|part|disc)\s*\d+\b|\b(?:disc)\b").unwrap();
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

    // result = DISC_REMOVAL.replace(&result, "").to_string();

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

fn series_cleanup(metadata: &mut FileScanCache) {
    if let Some(series) = &metadata.series {
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
}

fn author_cleanup(metadata: &mut FileScanCache) {
    let mut clean_author = String::new();
    if let Some(author) = &metadata.author {
        if is_dramatized(author) {
            metadata.dramatized = true;
        }
        clean_author = clean_metadata(&author).0;
    }
    if !clean_author.is_empty() {
        metadata.author = Some(clean_author);
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

pub async fn grouped_meta_cleanup(db: &SqlitePool) -> Result<(), ApiError> {
    //let grouped_data = group_meta_fetch(db).await?;
    // grouped_data.
    Ok(())
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
