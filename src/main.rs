use std::process;

use rustybookshelf;
use dotenv::dotenv;
fn main() {
    dotenv().ok();
    rustybookshelf::scan_for_audiobooks();
    
    if let Err(e) =  rustybookshelf::init_db() {
        eprintln!("Exiting program because of err");
        process::exit(1)
    }
}
