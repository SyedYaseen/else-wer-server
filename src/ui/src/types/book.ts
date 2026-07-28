// Mirrors src/models/audiobooks.rs::AudioBookRow (GET /api/list_books)
export interface AudioBookRow {
  id: number;
  author: string;
  series: string | null;
  title: string;
  files_location: string;
  duration: number;
  cover_art: string | null;
  metadata: string | null;
  book_size: number;
  series_id: number | null;
  series_sequence: string | null;
  asin: string | null;
  narrated_by: string | null;
  series_name: string | null;
  user_locked: boolean;
  description: string | null;
  series_locked: boolean;
  library_id: number | null;
  library_name: string | null;
}

// Mirrors src/models/user.rs::Progress (GET /api/list_inprogress etc.)
export interface Progress {
  id: number;
  user_id: number;
  book_id: number;
  file_id: number;
  progress_ms: number;
  complete: boolean;
  updated_at: string;
}

// Mirrors src/models/bookmarks.rs::Bookmark (POST/GET /api/bookmarks)
export interface Bookmark {
  id: number;
  user_id: number;
  book_id: number;
  file_id: number;
  timestamp_ms: number;
  note: string | null;
  created_at: string;
}

// Mirrors src/models/audiobooks.rs::FileMetadata (GET /api/file_metadata/{book_id})
export interface FileMetadata {
  id: number;
  book_id: number;
  file_id: number | null;
  file_name: string;
  file_path: string;
  duration: number | null;
  channels: number | null;
  sample_rate: number | null;
  bitrate: number | null;
  file_size: number | null;
  track_number: number | null;
}
