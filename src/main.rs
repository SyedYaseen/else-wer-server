use rustybookshelf;
use dotenv::dotenv;
fn main() {
    dotenv().ok();
    rustybookshelf::scan_for_audiobooks();
}
