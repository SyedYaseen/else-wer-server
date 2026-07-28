use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Serializes scan triggers (manual/upload/lazy/per-library) so they don't race each
/// other, while allowing genuinely independent work to proceed concurrently: two
/// different libraries' scans don't touch each other's `files_location`/`library_id`
/// rows or `scan_cache`, so there's no correctness reason to block one on the other.
/// A "full" scan (every library) still needs exclusivity against ALL per-library
/// scans, since it iterates the same library set a concurrent per-library scan might
/// be mid-way through.
struct ScanGuardState {
    full_scan_running: bool,
    scanning_libraries: HashSet<i64>,
}

pub struct ScanGuard(Mutex<ScanGuardState>);

impl ScanGuard {
    pub fn new() -> Self {
        Self(Mutex::new(ScanGuardState {
            full_scan_running: false,
            scanning_libraries: HashSet::new(),
        }))
    }

    /// Returns a guard that releases the full-scan lock on `Drop`. Move this into
    /// whatever task actually performs the scan (see callers) rather than dropping it
    /// at the end of the handler's stack frame: the guard must outlive the HTTP
    /// request so a client disconnect can't release the lock while the real scan
    /// work is still running in the background.
    pub fn try_start_full(self: &Arc<Self>) -> Option<FullScanGuard> {
        let mut state = self.0.lock().unwrap();
        if state.full_scan_running || !state.scanning_libraries.is_empty() {
            return None;
        }
        state.full_scan_running = true;
        Some(FullScanGuard(Arc::clone(self)))
    }

    /// See [`ScanGuard::try_start_full`] for why the returned guard must be moved
    /// into the scan's own task rather than scoped to the request handler.
    pub fn try_start_library(self: &Arc<Self>, library_id: i64) -> Option<LibraryScanGuard> {
        let mut state = self.0.lock().unwrap();
        if state.full_scan_running || state.scanning_libraries.contains(&library_id) {
            return None;
        }
        state.scanning_libraries.insert(library_id);
        Some(LibraryScanGuard {
            guard: Arc::clone(self),
            library_id,
        })
    }
}

impl Default for ScanGuard {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FullScanGuard(Arc<ScanGuard>);

impl Drop for FullScanGuard {
    fn drop(&mut self) {
        self.0.0.lock().unwrap().full_scan_running = false;
    }
}

pub struct LibraryScanGuard {
    guard: Arc<ScanGuard>,
    library_id: i64,
}

impl Drop for LibraryScanGuard {
    fn drop(&mut self) {
        self.guard.0.lock().unwrap().scanning_libraries.remove(&self.library_id);
    }
}
