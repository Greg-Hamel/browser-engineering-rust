use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::request::{HTTPMethod, HTTPRequest};

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

fn hash_from(request: HTTPRequest) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.url.as_str());

    request.headers.iter().for_each(|(key, value)| {
        hasher.update(format!("{}: {}", key, value));
    });

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

        Self::create_cache_control_file().unwrap();
    }

    fn clear() {
        fs::remove_dir_all(Self::get_cache_path()).expect("Cannot remove directory");

        Self::initialize_cache_dir()
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

    fn read_file(path_string: PathBuf) -> Vec<u8> {
        let mut file = OpenOptions::new().read(true).open(path_string).unwrap();

        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap();

        contents
    }

    pub fn extract(&self, request: &HTTPRequest) -> io::Result<Vec<u8>> {
        match request.method {
            HTTPMethod::GET => {
                for item in &self.items {
                    if item.path_string == hash_from(request.clone()) {
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
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("Method not supported"),
                ));
            }
        }
    }

    fn write_file(path: PathBuf, data: Vec<u8>) {
        // write data to file at path
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .unwrap();

        file.write_all(&data.as_slice()).unwrap();
    }

    pub fn insert(&mut self, request: &HTTPRequest, response: Vec<u8>, expiry: u64) {
        let file_name_hash = hash_from(request.clone());

        let file_path = Self::get_cache_path().join(&file_name_hash);

        Self::write_file(file_path, response);

        self.items.push(Item {
            expiry,
            path_string: file_name_hash,
        });

        Self::write_to_cache_control(&self)
    }

    pub fn initialize(clear_cache: bool) -> Cache {
        let cache_path = Self::get_cache_path();

        if !cache_path.is_dir() {
            Self::initialize_cache_dir();

            return Self { items: vec![] };
        } else {
            if clear_cache {
                Self::clear()
            }

            return Self {
                items: Self::read_cache_control(),
            };
        }
    }
}
