use std::collections::HashMap;
use std::fs;
use std::env;
use std::hash::Hash;
use std::io;

fn get_path() -> Result<String, env::VarError> {
    let key = "AUDIOBOOKS_LOCATION";
    env::var(key)
}

struct AudioBook {
    author: String,
    series: String,
    title: String,
    content_path: String
}

struct Series {

}

fn recursive_dirscan(path: &String, all_files: &mut HashMap<String, Vec<String>>) -> Result<(), io::Error> {
    let entries = fs::read_dir(path)?;
    for dir_entry in entries {
        let entry = dir_entry?;
        let f_name = entry.file_name();
        let f_type = entry.file_type()?;

        if f_type.is_dir() {
            let sub_dir = f_name.to_str().unwrap().to_string();
            let sub_dir_path = format!("{}/{}", path, sub_dir);
            recursive_dirscan(&sub_dir_path, all_files);
            

            let mut split_paths = sub_dir_path.split("/");
            split_paths.next();

            let author = split_paths.next().unwrap();
            let mut series = split_paths.next().unwrap();
            let mut title: &str = "";

            if let Some(val) = split_paths.next() {
                title = val;
            } else {
                title = series;
                series = "";
            }

            println!("Author: {author} series: {series} title: {title}");
            // println!("{}", f_name.display());
            // println!("{}", path);


            // println!("{sub_dir_path}");
        }

        // if f_type.is_file() {
        //     let mut split_paths = path.split("/");
        //     split_paths.next();

        //     let author = split_paths.next().unwrap();
        //     let mut series = split_paths.next().unwrap();
        //     let mut title: &str = "";

        //     if let Some(val) = split_paths.next() {
        //         title = val;
        //     } else {
        //         title = series;
        //         series = "";
        //     }

        //     println!("Author: {author} series: {series} title: {title}");          
        //     println!("{}", f_name.display());
        //     // println!("{}", path);
        // }
    }

    Ok(())

    // for entry in curr_dir.
    // let ftype = read_dir.file_type()?;
    // if ftype.is_dir() {
    //     recursive_dirscan(read_dir);
    // }
    // let  p = read_dir.path();
    // println!("{}", p.display());
}

// fn scan_base_dir(path: &String, audio_books: &mut Vec<AudioBook>) -> Result<(), io::Error> {
//     recursive_dirscan(path);
//     // let authors = fs::read_dir(path)?;

//     // for author_dir_res in authors {
//     //     let author_dir = author_dir_res?;
//     //     let author_type = author_dir.file_type()?;

        
//     //     recursive_dirscan(author_dir);   
        

//         // let author_name = author.file_name();
        
        
        
        
//         // if let Ok(ft) = author.file_type() {
//         //     if ft.is_dir() {
//         //         // Drill down
//         //     } else {
//         //         // Collect files into vector
//         //     }
//         // }
        

//         // match author {
//         //     Ok(author) => {
//         //         println!("{:#?} {:#?}", author.file_name(), author.file_type());
//         //         let book: AudioBook = {
//         //             author: author.file_name(),
//         //             series: String::from(""),

//         //         };

//         //     } 
//         //     Err(e) => {
//         //         println!("Err at location {}", e);
//         //         return;
//         //     }
//         // };
//     // }
//     Ok(())
// }

pub fn scan_for_audiobooks() -> Result<(), env::VarError> {
    let path = get_path()?;

    let mut audio_books: Vec<AudioBook> = Vec::new();
    // scan_base_dir(&path, &mut audio_books);
    let mut all_files: HashMap<String,Vec<String>> = HashMap::new();
    recursive_dirscan(&path, &mut all_files);
    Ok(())
}

