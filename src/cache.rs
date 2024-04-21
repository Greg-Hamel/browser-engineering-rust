use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::uri::URI;

const CACHE_PATH: &str = ".cache/";

#[derive(Clone)]
struct Item {
    expiry: u64,
    path_string: String,
}

pub struct Cache {
    items: Vec<Item>,
}

fn extract_item_from(string_vec: Vec<&str>) -> Item {
    Item {
        expiry: string_vec[0].parse().unwrap(),
        path_string: String::from(string_vec[1]),
    }
}

fn hash_from(uri: &URI) -> String {
    let mut hasher = Sha256::new();
    hasher.update(uri.path.as_bytes());

    match &uri.authority {
        Some(authority) => {
            hasher.update(authority.host.as_bytes());

            hasher.update(authority.port.to_string().as_bytes());
        }
        None => (),
    }

    let value = hasher.finalize();

    return format!("{:X}", value);
}

impl Cache {
    const BASE_PATH: &str = CACHE_PATH;
    const CONTROL_FILE: &str = ".control";

    fn get_cache_path() -> &'static Path {
        Path::new(Self::BASE_PATH)
    }

    fn create_cache_control_file() -> io::Result<File> {
        File::create(Self::get_cache_path().join(Self::CONTROL_FILE))
    }

    fn initialize_cache_dir() {
        let cache_path = Self::get_cache_path();

        match fs::create_dir(cache_path) {
            Ok(..) => (),
            Err(e) => panic!("{e}"),
        }

        Self::create_cache_control_file();
    }

    fn read_cache_control() -> Vec<Item> {
        let control_file = Self::get_cache_path().join(Self::CONTROL_FILE);

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(control_file)
            .expect("File cannot be opened");

        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        let mut items: Vec<Item> = Vec::new();

        for line in contents.lines() {
            let line_collection: Vec<&str> = line.split(';').collect();
            let data = extract_item_from(line_collection);

            items.push(data);
        }

        items
    }

    fn write_to_cache_control(&self) {
        // Function to write to cache control
        let control_file = Self::get_cache_path().join(Self::CONTROL_FILE);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(control_file)
            .unwrap();
        let items: Vec<String> = self
            .items
            .iter()
            .map(|item| format!("{};{}", item.expiry, item.path_string))
            .collect();
        let data = items.join("\n");
        file.write_all(data.as_bytes()).unwrap();
    }

    fn read_file(path_string: PathBuf) -> String {
        let mut file = OpenOptions::new().read(true).open(path_string).unwrap();

        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        contents
    }

    pub fn extract(&mut self, uri: &URI) -> io::Result<String> {
        for item in &self.items {
            if item.path_string == hash_from(&uri) {
                // TODO: implement expiry check
                let file_path = Self::get_cache_path().join(&item.path_string);
                return Ok(Self::read_file(file_path));
            }
        }

        return Err(Error::new(
            ErrorKind::NotFound,
            format!("Value not found in cache"),
        ));
    }

    fn write_file(path: PathBuf, data: String) {
        // write data to file at path
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .unwrap();

        file.write_all(data.as_bytes()).unwrap();
    }

    pub fn insert(&mut self, uri: &URI, data: String, expiry: u64) {
        let file_name_hash = hash_from(uri);

        let file_path = Self::get_cache_path().join(&file_name_hash);

        Self::write_file(file_path, data);

        self.items.push(Item {
            expiry,
            path_string: file_name_hash,
        });

        Self::write_to_cache_control(&self)
    }

    pub fn initialize() -> Cache {
        let cache_path = Self::get_cache_path();

        if !cache_path.is_dir() {
            Self::initialize_cache_dir();

            return Self { items: vec![] };
        } else {
            return Self {
                items: Self::read_cache_control(),
            };
        }
    }
}
