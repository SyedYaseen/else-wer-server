use std::collections::HashMap;
use std::fs;
use std::env;
use std::hash::Hash;
use std::io;

fn get_path() -> Result<String, env::VarError> {
    let key = "AUDIOBOOKS_LOCATION";
    env::var(key)
}

#[derive(Debug)]
struct AudioBook {
    author: String,
    series: String,
    title: String,
    content_path: String
}

impl AudioBook {
    fn new(author: String, series: String, title: String, content_path: String ) -> AudioBook {
        AudioBook {
            author: author,
            series: series,
            title: title,
            content_path: content_path
        }
    }
}



fn recursive_dirscan(path: &String, audio_books: &mut Vec<AudioBook>) -> Result<(), io::Error> {
    let entries = fs::read_dir(path)?;
    for dir_entry in entries {
        let entry = dir_entry?;
        let f_name = entry.file_name();
        let f_type = entry.file_type()?;

        if f_type.is_dir() {
            let sub_dir = f_name.to_str().unwrap().to_string();
            let sub_dir_path = format!("{}/{}", path, sub_dir);
            if let Err(e) =  recursive_dirscan(&sub_dir_path, audio_books) {
                println!("Err reading folder {e}");
            }
            
            let mut split_paths = sub_dir_path.split("/");
            split_paths.next();

            let author = match split_paths.next() {
                Some(v) => v,
                None => continue
            };

            let mut series = match split_paths.next() {
                Some(v) => v,
                None => continue
            };

            let title = match split_paths.next() {
                Some(v) => v,
                None => {
                    let title = series;
                    series = "";
                    title
                }
            };

            if let Some(a) = audio_books.last() {
                if a.series == title {
                    continue;
                }
            }
 
            audio_books.push(
                AudioBook::new(author.to_owned(), series.to_owned(), title.to_owned(), sub_dir_path)
            );
        }
    }

    Ok(())
}

pub fn scan_for_audiobooks() -> Result<(), env::VarError> {
    let path = get_path()?;

    let mut audio_books: Vec<AudioBook> = Vec::new();
    // scan_base_dir(&path, &mut audio_books);
    let mut audio_books: Vec<AudioBook> = Vec::new();
    recursive_dirscan(&path, &mut audio_books);

    for i in &audio_books {
        println!("{:#?}", i);
    }
    Ok(())
}

