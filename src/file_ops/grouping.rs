use crate::file_ops::meta_cleanup::fold_key;
use std::collections::HashMap;
use std::path::Path;
use strsim::levenshtein;

/// One scan-pass file record, as fed into the grouping pass. `row_idx` indexes back
/// into the in-memory `FileScanCache` chunk slice (no DB staging table since 0008).
#[derive(Debug, Clone)]
pub struct GroupRow {
    pub row_idx: usize,
    pub author: Option<String>,
    pub narrated_by: Option<String>,
    /// Album-derived; falls back to clean_title upstream when the file has no album tag.
    pub clean_series: Option<String>,
    pub clean_title: Option<String>,
    pub path_parent: String,
    pub cover_art: Option<String>,
}

/// One resolved book: a folder, or one album-partition of a folder holding loose files.
#[derive(Debug, Clone)]
pub struct BookGroup {
    pub path_parent: String,
    pub title: String,
    pub author: String,
    /// Album/display value — the audiobooks.series column keeps this name.
    pub series: String,
    pub narrated_by: Option<String>,
    pub cover_art: Option<String>,
    pub row_indices: Vec<usize>,
}

pub const PREFIX_MERGE_RATIO: f64 = 0.5;
pub const LEVENSHTEIN_MERGE_SIM: f64 = 0.75;
pub const DOMINANT_PARTITION_RATIO: f64 = 0.5;

/// Strip a leading article and trailing order digits/punctuation so spelling
/// variants of one album ("the fellowship of the ring" / "fellowship of the ring 01")
/// compare equal.
fn normalize_album_key(k: &str) -> String {
    let mut s = k.trim();
    for article in ["the ", "a ", "an "] {
        if let Some(rest) = s.strip_prefix(article) {
            s = rest;
            break;
        }
    }
    s.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace() || c.is_ascii_punctuation())
        .to_string()
}

/// True when two folded album keys name the same book despite tag inconsistencies.
pub fn keys_similar(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }

    let (a, b) = (normalize_album_key(a), normalize_album_key(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    // Containment only for keys long enough to be distinctive.
    let min_len = a.chars().count().min(b.chars().count());
    if min_len >= 5 && (a.contains(&b) || b.contains(&a)) {
        return true;
    }

    let shorter = a.chars().count().min(b.chars().count());
    let longer = a.chars().count().max(b.chars().count());

    let shared_prefix = a
        .chars()
        .zip(b.chars())
        .take_while(|(x, y)| x == y)
        .count();
    if shared_prefix as f64 / shorter as f64 >= PREFIX_MERGE_RATIO {
        return true;
    }

    let sim = 1.0 - levenshtein(&a, &b) as f64 / longer as f64;
    sim >= LEVENSHTEIN_MERGE_SIM
}

/// Most frequent non-None value; ties broken by first occurrence.
fn mode<'a, I: Iterator<Item = Option<&'a String>>>(iter: I) -> Option<String> {
    let mut counts: Vec<(&'a String, usize)> = Vec::new();
    for v in iter.flatten() {
        match counts.iter_mut().find(|(k, _)| *k == v) {
            Some((_, c)) => *c += 1,
            None => counts.push((v, 1)),
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(v, _)| v.clone())
}

struct Partition {
    key: String, // folded album key; empty = no album
    rows: Vec<GroupRow>,
}

/// Group scan-cache rows into books. Folder = identity; within a folder, album-tag
/// partitions merge when similar or when they are junk (album == author/narrator, or
/// missing) and one partition dominates. Surviving partitions are separate books
/// (the "N loose single-file books in one folder" case).
pub fn group_files(rows: Vec<GroupRow>) -> Vec<BookGroup> {
    let mut folders: HashMap<String, Vec<GroupRow>> = HashMap::new();
    for row in rows {
        folders.entry(row.path_parent.clone()).or_default().push(row);
    }

    let mut groups: Vec<BookGroup> = Vec::new();
    let mut folder_list: Vec<(String, Vec<GroupRow>)> = folders.into_iter().collect();
    folder_list.sort_by(|a, b| a.0.cmp(&b.0));

    for (path_parent, folder_rows) in folder_list {
        // Split rows into junk (no album, or album is just the author/narrator name —
        // it can't identify a book) and real album rows.
        let (junk_rows, real_rows): (Vec<GroupRow>, Vec<GroupRow>) =
            folder_rows.into_iter().partition(is_junk_row);

        // Sub-partition real rows by folded album key.
        let mut partitions: Vec<Partition> = Vec::new();
        for row in real_rows {
            let key = fold_key(row.clean_series.as_deref().unwrap_or_default());
            match partitions.iter_mut().find(|p| p.key == key) {
                Some(p) => p.rows.push(row),
                None => partitions.push(Partition { key, rows: vec![row] }),
            }
        }

        // Merge similar partitions until fixpoint.
        loop {
            let mut merged = false;
            'outer: for i in 0..partitions.len() {
                for j in (i + 1)..partitions.len() {
                    if keys_similar(&partitions[i].key, &partitions[j].key) {
                        let absorbed = partitions.remove(j);
                        partitions[i].rows.extend(absorbed.rows);
                        merged = true;
                        break 'outer;
                    }
                }
            }
            if !merged {
                break;
            }
        }

        // Place junk rows: with exactly one real book in the folder they belong to it
        // (Hobbit: 18x album=narrator + 1 real album). With several real books, only a
        // dominant one (>= half the folder) absorbs them. Otherwise they form their own
        // partition and fall back to per-title naming.
        if !junk_rows.is_empty() {
            let total: usize =
                partitions.iter().map(|p| p.rows.len()).sum::<usize>() + junk_rows.len();
            let target = match partitions.len() {
                0 => None,
                1 => Some(0),
                _ => partitions
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        p.rows.len() as f64 / total as f64 >= DOMINANT_PARTITION_RATIO
                    })
                    .max_by_key(|(_, p)| p.rows.len())
                    .map(|(i, _)| i),
            };
            match target {
                Some(i) => partitions[i].rows.extend(junk_rows),
                None => partitions.push(Partition {
                    key: String::new(),
                    rows: junk_rows,
                }),
            }
        }

        for partition in partitions {
            groups.push(partition_to_group(&path_parent, partition));
        }
    }

    groups
}

/// A row whose album is missing or just names the author/narrator can't identify a book.
fn is_junk_row(r: &GroupRow) -> bool {
    let Some(album) = r.clean_series.as_deref() else {
        return true;
    };
    if album.trim().is_empty() {
        return true;
    }
    let key = fold_key(album);
    r.author.as_deref().map(fold_key).as_deref() == Some(&key)
        || r.narrated_by.as_deref().map(fold_key).as_deref() == Some(&key)
}

fn partition_to_group(path_parent: &str, partition: Partition) -> BookGroup {
    let rows = partition.rows;
    let file_count = rows.len();

    // Junk albums folded into this partition must not outvote the real album name.
    let album_mode = mode(
        rows.iter()
            .filter(|r| !is_junk_row(r))
            .map(|r| r.clean_series.as_ref()),
    )
    .or_else(|| mode(rows.iter().map(|r| r.clean_series.as_ref())));

    let title = if file_count > 1 {
        album_mode.clone()
    } else {
        rows[0].clean_title.clone().or_else(|| album_mode.clone())
    }
    .unwrap_or_else(|| {
        // Last resort: name the book after its folder.
        Path::new(path_parent)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_parent.to_string())
    });

    let author = mode(
        rows.iter()
            .filter(|r| !is_junk_row(r))
            .map(|r| r.author.as_ref()),
    )
    .or_else(|| mode(rows.iter().map(|r| r.author.as_ref())))
    .unwrap_or_else(|| "unknown".into());
    let series = album_mode.unwrap_or_else(|| title.clone());
    let narrated_by = mode(rows.iter().map(|r| r.narrated_by.as_ref()));
    let cover_art = rows.iter().find_map(|r| r.cover_art.clone());

    BookGroup {
        path_parent: path_parent.to_string(),
        title,
        author,
        series,
        narrated_by,
        cover_art,
        row_indices: rows.iter().map(|r| r.row_idx).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        row_idx: usize,
        author: &str,
        album: Option<&str>,
        title: Option<&str>,
        parent: &str,
    ) -> GroupRow {
        GroupRow {
            row_idx,
            author: Some(author.to_string()),
            narrated_by: None,
            clean_series: album.map(|s| s.to_string()),
            clean_title: title.map(|s| s.to_string()),
            path_parent: parent.to_string(),
            cover_art: None,
        }
    }

    #[test]
    fn single_folder_single_album_one_book() {
        // Húrin: 128 chapter files, one album, per-track titles.
        let rows: Vec<GroupRow> = (0..128)
            .map(|i| {
                row(
                    i,
                    "j.r.r. tolkien",
                    Some("the children of húrin"),
                    Some(&format!("chapter {i}")),
                    "/lib/hurin/MP3",
                )
            })
            .collect();
        let groups = group_files(rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title, "the children of húrin");
        assert_eq!(groups[0].row_indices.len(), 128);
    }

    #[test]
    fn junk_album_folds_into_dominant() {
        // Hobbit: 18 files with album = narrator name (== author tag), 1 real album.
        // Junk-partition detection compares the album key against the row's author.
        let mut rows: Vec<GroupRow> = (0..18)
            .map(|i| {
                row(
                    i,
                    "rob inglis",
                    Some("rob inglis"),
                    Some(&format!("track {i}")),
                    "/lib/hobbit/MP31",
                )
            })
            .collect();
        rows.push(row(
            18,
            "j.r.r. tolkien",
            Some("the hobbit"),
            Some("track 19"),
            "/lib/hobbit/MP31",
        ));
        let groups = group_files(rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].row_indices.len(), 19);
        assert_eq!(groups[0].title, "the hobbit");
        assert_eq!(groups[0].author, "j.r.r. tolkien");
    }

    #[test]
    fn similar_album_variants_merge() {
        // Fellowship MP3 folder: two album spellings of one book.
        let mut rows: Vec<GroupRow> = (0..20)
            .map(|i| {
                row(
                    i,
                    "j.r.r. tolkien",
                    Some("the fellowship of the ring"),
                    Some(&format!("ch {i}")),
                    "/lib/fellowship/MP3",
                )
            })
            .collect();
        for i in 20..31 {
            rows.push(row(
                i,
                "j.r.r. tolkien",
                Some("fellowship of the ring 01"),
                Some(&format!("ch {i}")),
                "/lib/fellowship/MP3",
            ));
        }
        let groups = group_files(rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].row_indices.len(), 31);
    }

    #[test]
    fn different_path_parents_stay_separate() {
        // m4b parts + MP3 chapter subfolder = 2 books.
        let mut rows = vec![
            row(0, "a", Some("fellowship"), Some("part 1"), "/lib/fellowship"),
            row(1, "a", Some("fellowship"), Some("part 2"), "/lib/fellowship"),
            row(2, "a", Some("fellowship"), Some("part 3"), "/lib/fellowship"),
        ];
        for i in 3..34 {
            rows.push(row(
                i,
                "a",
                Some("fellowship"),
                Some(&format!("ch {i}")),
                "/lib/fellowship/MP3",
            ));
        }
        let groups = group_files(rows);
        assert_eq!(groups.len(), 2);
        let mut sizes: Vec<usize> = groups.iter().map(|g| g.row_indices.len()).collect();
        sizes.sort();
        assert_eq!(sizes, vec![3, 31]);
    }

    #[test]
    fn loose_files_partition_by_title() {
        // 4 dissimilar albums (or clean_title fallback) in one folder = 4 books.
        let rows = vec![
            row(0, "a", Some("dune"), Some("dune"), "/lib/loose"),
            row(1, "a", Some("neuromancer"), Some("neuromancer"), "/lib/loose"),
            row(2, "a", Some("hyperion cantos"), Some("hyperion"), "/lib/loose"),
            row(3, "a", Some("the martian by weir"), Some("the martian"), "/lib/loose"),
        ];
        let groups = group_files(rows);
        assert_eq!(groups.len(), 4);
        for g in &groups {
            assert_eq!(g.row_indices.len(), 1);
            assert_eq!(g.path_parent, "/lib/loose");
        }
    }

    #[test]
    fn single_file_uses_clean_title() {
        let rows = vec![row(0, "a", Some("some album"), Some("real title"), "/lib/x")];
        let groups = group_files(rows);
        assert_eq!(groups[0].title, "real title");
        assert_eq!(groups[0].series, "some album");
    }

    #[test]
    fn diacritic_variants_group_together() {
        let rows = vec![
            row(0, "a", Some("the children of húrin"), Some("c1"), "/lib/h"),
            row(1, "a", Some("the children of hurin"), Some("c2"), "/lib/h"),
        ];
        let groups = group_files(rows);
        assert_eq!(groups.len(), 1);
    }
}
