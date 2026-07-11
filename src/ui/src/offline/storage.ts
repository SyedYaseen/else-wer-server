import { streamUrl } from '../api/progress';
import type { AudioBookRow, FileMetadata } from '../types/book';

// IndexedDB, not Cache API: Cache Storage requires a secure context (HTTPS or
// localhost), which this self-hosted app doesn't have on a plain-HTTP LAN
// deployment. IndexedDB has no such restriction and can store Blobs directly.
const DB_NAME = 'else-wer-offline';
const DB_VERSION = 3;
const STORE = 'files';
const BOOKS_STORE = 'books';
const SEGMENT_STORE = 'segments';

// Audio is stored as ~8MB segments written to IndexedDB as they arrive, never
// holding a whole file in JS memory — buffering a full audiobook file blew
// past iOS Safari's per-tab memory limit and crash-reloaded the page mid-
// download (wiping all in-memory state, which looked like the download
// silently resetting).
const SEGMENT_BYTES = 8 * 1024 * 1024;

// Metadata marker, written only after every segment of the file has been
// stored — its presence means the file is completely downloaded.
interface StoredFile {
  fileId: number;
  bookId: number;
  mime?: string;
  // Legacy DB v2 records stored the whole file inline; still readable.
  blob?: Blob;
}

interface StoredSegment {
  fileId: number;
  seq: number;
  blob: Blob;
}

// Snapshot of a downloaded book's metadata, so BookDetailPage/PlayerPage can
// render and start playback without a network round-trip when offline.
export interface StoredBook {
  bookId: number;
  book: AudioBookRow;
  files: FileMetadata[];
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE)) {
        const store = db.createObjectStore(STORE, { keyPath: 'fileId' });
        store.createIndex('bookId', 'bookId');
      }
      if (!db.objectStoreNames.contains(BOOKS_STORE)) {
        db.createObjectStore(BOOKS_STORE, { keyPath: 'bookId' });
      }
      if (!db.objectStoreNames.contains(SEGMENT_STORE)) {
        db.createObjectStore(SEGMENT_STORE, { keyPath: ['fileId', 'seq'] });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function withStoreNamed<T>(
  storeName: string,
  mode: IDBTransactionMode,
  fn: (store: IDBObjectStore) => IDBRequest<T>,
): Promise<T> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const store = db.transaction(storeName, mode).objectStore(storeName);
    const req = fn(store);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function withStore<T>(mode: IDBTransactionMode, fn: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return withStoreNamed(STORE, mode, fn);
}

function withBooksStore<T>(mode: IDBTransactionMode, fn: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return withStoreNamed(BOOKS_STORE, mode, fn);
}

function withSegments<T>(mode: IDBTransactionMode, fn: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return withStoreNamed(SEGMENT_STORE, mode, fn);
}

function segmentRange(fileId: number): IDBKeyRange {
  return IDBKeyRange.bound([fileId, 0], [fileId, Number.MAX_SAFE_INTEGER]);
}

function deleteSegments(fileId: number): Promise<undefined> {
  return withSegments('readwrite', (store) => store.delete(segmentRange(fileId)));
}

function putFile(entry: StoredFile): Promise<IDBValidKey> {
  return withStore('readwrite', (store) => store.put(entry));
}

function getFile(fileId: number): Promise<StoredFile | undefined> {
  return withStore('readonly', (store) => store.get(fileId));
}

function getFilesByBook(bookId: number): Promise<StoredFile[]> {
  return withStore('readonly', (store) => store.index('bookId').getAll(bookId));
}

async function deleteFilesByBook(bookId: number): Promise<void> {
  const fileIds: number[] = [];
  const db = await openDb();
  await new Promise<void>((resolve, reject) => {
    const store = db.transaction(STORE, 'readwrite').objectStore(STORE);
    const req = store.index('bookId').openCursor(IDBKeyRange.only(bookId));
    req.onsuccess = () => {
      const cursor = req.result;
      if (cursor) {
        fileIds.push((cursor.value as StoredFile).fileId);
        cursor.delete();
        cursor.continue();
      } else {
        resolve();
      }
    };
    req.onerror = () => reject(req.error);
  });
  for (const fileId of fileIds) await deleteSegments(fileId);
}

export async function downloadBook(
  book: AudioBookRow,
  files: FileMetadata[],
  onProgress?: (fraction: number) => void,
): Promise<void> {
  const sorted = files.slice().sort((a, b) => (a.track_number ?? 0) - (b.track_number ?? 0));
  const totalBytes = sorted.reduce((sum, f) => sum + (f.file_size ?? 0), 0);

  let loadedBytes = 0;
  for (const file of sorted) {
    // Clear any partial data from an earlier interrupted attempt.
    await deleteSegments(file.id);

    const res = await fetch(streamUrl(file.id));
    if (!res.ok || !res.body) throw new Error(`Download failed: ${res.status}`);
    const mime = res.headers.get('Content-Type') ?? undefined;

    const reader = res.body.getReader();
    let chunks: BlobPart[] = [];
    let pendingBytes = 0;
    let seq = 0;

    const flush = async () => {
      if (chunks.length === 0) return;
      const segment: StoredSegment = { fileId: file.id, seq: seq++, blob: new Blob(chunks) };
      chunks = [];
      pendingBytes = 0;
      await withSegments('readwrite', (store) => store.put(segment));
    };

    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      pendingBytes += value.byteLength;
      loadedBytes += value.byteLength;
      if (pendingBytes >= SEGMENT_BYTES) await flush();
      if (totalBytes > 0) onProgress?.(Math.min(1, loadedBytes / totalBytes));
    }
    await flush();

    // Marker last: its presence means every segment above is already stored.
    await putFile({ fileId: file.id, bookId: book.id, mime });
  }

  await withBooksStore('readwrite', (store) => store.put({ bookId: book.id, book, files } satisfies StoredBook));
  onProgress?.(1);
}

export async function getOfflineBookData(
  bookId: number,
): Promise<{ book: AudioBookRow; files: FileMetadata[] } | null> {
  const entry = await withBooksStore<StoredBook | undefined>('readonly', (store) => store.get(bookId));
  return entry ? { book: entry.book, files: entry.files } : null;
}

export async function isBookDownloaded(bookId: number, files: FileMetadata[]): Promise<boolean> {
  if (files.length === 0) return false;
  const stored = await getFilesByBook(bookId);
  const storedIds = new Set(stored.map((s) => s.fileId));
  return files.every((f) => storedIds.has(f.id));
}

export async function getOfflineFileBlob(fileId: number): Promise<Blob | null> {
  const entry = await getFile(fileId);
  if (!entry) return null;
  if (entry.blob) return entry.blob; // legacy v2 whole-file record
  const segments = await withSegments<StoredSegment[]>('readonly', (store) => store.getAll(segmentRange(fileId)));
  if (segments.length === 0) return null;
  segments.sort((a, b) => a.seq - b.seq);
  // Blobs read back from IndexedDB are disk-backed; concatenating them into
  // one Blob is a cheap reference operation, not a copy into JS memory.
  return new Blob(segments.map((s) => s.blob), { type: entry.mime });
}

export async function deleteBookDownload(bookId: number): Promise<void> {
  await deleteFilesByBook(bookId);
  await withBooksStore('readwrite', (store) => store.delete(bookId));
}

export async function listDownloadedBooks(): Promise<StoredBook[]> {
  return withBooksStore<StoredBook[]>('readonly', (store) => store.getAll());
}

export async function clearAllDownloads(): Promise<void> {
  const db = await openDb();
  await Promise.all(
    [STORE, BOOKS_STORE, SEGMENT_STORE].map(
      (name) =>
        new Promise<void>((resolve, reject) => {
          const req = db.transaction(name, 'readwrite').objectStore(name).clear();
          req.onsuccess = () => resolve();
          req.onerror = () => reject(req.error);
        }),
    ),
  );
}
