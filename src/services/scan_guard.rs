use std::collections::HashSet;
use std::sync::Mutex;

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

    pub fn try_start_full(&self) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.full_scan_running || !state.scanning_libraries.is_empty() {
            return false;
        }
        state.full_scan_running = true;
        true
    }

    pub fn finish_full(&self) {
        self.0.lock().unwrap().full_scan_running = false;
    }

    pub fn try_start_library(&self, library_id: i64) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.full_scan_running || state.scanning_libraries.contains(&library_id) {
            return false;
        }
        state.scanning_libraries.insert(library_id);
        true
    }

    pub fn finish_library(&self, library_id: i64) {
        self.0.lock().unwrap().scanning_libraries.remove(&library_id);
    }
}

impl Default for ScanGuard {
    fn default() -> Self {
        Self::new()
    }
}
