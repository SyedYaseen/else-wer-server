use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref BRACKETS: Regex = Regex::new(r"(?i)\([^)]*\)|\[[^\]]*\]|\{[^}]*\}").unwrap();
    static ref TRAILING_SEPARATORS: Regex = Regex::new(r"(?i)\s*(-|–|—|::|by)\s*$").unwrap();
    static ref COMMON_TERMS: Regex = Regex::new(r"(?i)\b(unabridged|abridged|audible|original|special edition|dramatized|read by|narrated by|a novel)\b").unwrap();
    static ref MULTIPLE_SPACES: Regex = Regex::new(r"\s{2,}").unwrap();
}

pub fn clean_metadata(word: &str) -> String {
    let mut result = word.to_string();

    // Remove content in parentheses, brackets, and braces
    result = BRACKETS.replace_all(&result, "").to_string();

    // Remove trailing separators
    result = TRAILING_SEPARATORS.replace_all(&result, "").to_string();

    // Remove common audiobook terms
    result = COMMON_TERMS.replace_all(&result, "").to_string();

    // Trim and clean up whitespace
    result = result.trim().to_string();
    result = MULTIPLE_SPACES.replace_all(&result, " ").to_string();

    result
}
