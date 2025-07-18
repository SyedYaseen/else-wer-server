use std::fs;
use std::env;

fn get_path() -> Result<String, env::VarError> {
    let key = "AUDIOBOOKS_LOCATION";
    let path = match env::var(key) {
        Ok(val) => val,
        Err(e) => {
            println!("Env {} not found. {}", key, e);
            return Err(e)
        }
    };
    Ok(path)
}

fn scan_dir_recursively(path: &String) {
    let mut entries = match  fs::read_dir(path) {
        Ok(r) => r,
        Err(e) => { 
            println!("Err while reading path {}", e);
            return;
        }
    };

    for entry in entries {
        match entry {
            Ok(entry) => println!("{:#?} {:#?}", entry.file_name(), entry.file_type()) ,
            Err(e) => {
                println!("Err at location {}", e);
                return;
            }
        };
    }
}

pub fn scan_for_audiobooks() -> Result<(), env::VarError> {
    let path = get_path()?;

    println!("{}", path);
    scan_dir_recursively(&path);
    Ok(())
}

