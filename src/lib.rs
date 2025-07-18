mod storage;
use storage::fs;
pub fn scan_for_audiobooks() {
    fs::scan_for_audiobooks();

}