// Mirrors src/models/meta_scan.rs FileInfo/ChangeDto and src/models/match_meta.rs.

export interface FileInfo {
  id: number; // file_scan_cache PK (NOT files.id used for streaming)
  book_id: number;
  author: string;
  title: string;
  series: string;
  file_path: string;
  path_parent: string;
  file_name: string;
}

// GET /api/list_scanned_files -> { files: author -> series -> FileInfo[] }
export type GroupedFiles = Record<string, Record<string, FileInfo[]>>;

export type ChangeType = 'rename' | 'move-title' | 'merge-title' | 'file-move';

export interface ChangeDto {
  change_type: ChangeType;
  file_ids: number[];
  current_book_ids?: number[];
  new_book_id?: number;
  current_author?: string;
  current_series?: string;
  current_filetitle?: string;
  new_author?: string;
  new_series?: string;
  new_filetitle?: string;
}

export interface MatchCandidate {
  title: string;
  author?: string;
  narrator?: string;
  series_name?: string;
  series_sequence?: string;
  series_asin?: string;
  year?: number;
  asin?: string;
  cover_url?: string;
  description?: string;
  confidence: number;
}

export interface MatchResponse {
  book_id: number;
  book_title: string;
  book_author: string;
  book_cover_art?: string | null;
  candidates: MatchCandidate[];
}

export interface ApplyMatchDto extends MatchCandidate {
  apply_title_author: boolean;
  apply_cover: boolean;
}

// ── Client-side tree, derived from GroupedFiles ─────────────────────────
// A (author, series) bucket from the server can mix files from more than one
// book_id (e.g. the "unknown" series fallback), so books are re-grouped by
// book_id on the client for a correct 3-level author -> book -> file tree.

export interface BookGroup {
  bookId: number;
  author: string;
  series: string;
  title: string;
  files: FileInfo[];
}

export interface AuthorGroup {
  author: string;
  books: BookGroup[];
}

// Groups by a case-folded key so differently-cased scans of the same real
// author (e.g. "brandon sanderson" vs "Brandon Sanderson") collapse into one
// row here, regardless of what's actually stored per-book in the DB.
export function buildTree(grouped: GroupedFiles): AuthorGroup[] {
  const byKey = new Map<string, AuthorGroup>();

  for (const [author, seriesMap] of Object.entries(grouped)) {
    const key = author.trim().toLowerCase();
    let authorGroup = byKey.get(key);
    if (!authorGroup) {
      authorGroup = { author, books: [] };
      byKey.set(key, authorGroup);
    }
    const booksByBookId = new Map<number, BookGroup>(authorGroup.books.map((b) => [b.bookId, b]));

    for (const [series, files] of Object.entries(seriesMap)) {
      for (const file of files) {
        let group = booksByBookId.get(file.book_id);
        if (!group) {
          group = { bookId: file.book_id, author: authorGroup.author, series, title: file.title, files: [] };
          booksByBookId.set(file.book_id, group);
          authorGroup.books.push(group);
        }
        group.files.push(file);
      }
    }
  }

  for (const authorGroup of byKey.values()) {
    authorGroup.books.sort((a, b) => a.title.localeCompare(b.title));
  }

  return Array.from(byKey.values()).sort((a, b) => a.author.localeCompare(b.author));
}
