use crate::logger::CONSOLE_LOGGER;
use crate::request::{Request, RequestOptions};
use crate::uri::Scheme;
use crate::uri::URI;

use core::panic;
use log::LevelFilter;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;

pub mod cache;
pub mod logger;
pub mod request;
pub mod uri;

struct Options {
    debug: bool,
    url: String,
    clear_cache: bool,
}

struct Browser {
    options: Options,
    request: Request,
}

impl Browser {
    pub fn new(options: Options) -> Self {
        let cache = cache::Cache::initialize(options.clear_cache);
        let requester = Request::init(RequestOptions { cache });
        Self {
            options,
            request: requester,
        }
    }

    fn transform(&mut self, data: &str) -> String {
        let lt_re = Regex::new(r"<").unwrap();
        let gt_re = Regex::new(r">").unwrap();

        let no_lt = String::from(lt_re.replace_all(data, "&lt;"));

        String::from(gt_re.replace_all(&no_lt.as_str(), "&gt;"))
    }

    fn show(&mut self, source: &str, only_body: bool) {
        let mut in_angle = false;
        let mut in_body = false;

        let html_entities = HashMap::from([("&lt;", "<"), ("&gt;", ">")]);

        let mut current_tag = String::new();
        let mut possible_entity = String::new();

        for character in source.chars() {
            if character == '<' {
                in_angle = true
            } else if character == '>' {
                if current_tag == "body" {
                    in_body = true
                } else if current_tag == "/body" {
                    in_body = false
                }
                current_tag = String::new();
                in_angle = false
            } else if !in_angle {
                if only_body && !in_body {
                    // way to show only inside the body element
                    continue;
                }

                if character == '&' || possible_entity.len() > 0 {
                    // HTML entity interpretation
                    if character == '&' && possible_entity.len() == 0 {
                        possible_entity += &character.to_string();
                    } else if possible_entity.len() > 0 {
                        if possible_entity.len() > 25 {
                            // No entity has an allowable name space large than 23 + 2, dump current buffer.
                            print!("{possible_entity}");
                            possible_entity = String::new();
                            continue;
                        }

                        possible_entity += &character.to_string();

                        if character == ';' {
                            if html_entities.contains_key(&possible_entity.as_str()) {
                                let string_value =
                                    html_entities.get(&possible_entity.as_str()).unwrap_or(&"");
                                print!("{}", string_value)
                            } else {
                                print!("{possible_entity}")
                            }

                            possible_entity = String::new();
                        }
                    }

                    continue;
                }

                print!("{character}")
            } else if in_angle {
                current_tag += &character.to_string();
            }
        }

        if possible_entity.len() > 0 {
            // If buffer still full, dump its content
            print!("{possible_entity}");
            possible_entity = String::new();
        }
    }

    fn load(&mut self) {
        let uri = URI::parse(&self.options.url);

        match uri.scheme {
            Scheme::HTTPS | Scheme::HTTP => {
                let response = self.request.send(&uri).expect("Couldn't parse response...");

                if uri.flags.contains_key(&String::from("view-source")) {
                    let transformed_response = self.transform(&response.data.as_str());
                    self.show(&transformed_response, false)
                } else {
                    self.show(&response.data, true)
                }
            }
            Scheme::Data => {
                // _ is the content_type
                let (_, path_data) = uri.path.split_once(',').unwrap_or((&uri.path, ""));

                // Writing end-of-file.
                let data = String::new() + path_data + "\r\n";
                self.show(&data, false)
            }
            Scheme::File => {
                let data = fs::read_to_string(&uri.path).expect("File not found...");
                self.show(&data, false)
            }
            Scheme::VIEWSOURCE => panic!("Unexpected view-source scheme provided to browser."),
        }
    }

    pub fn run(&mut self) {
        self.load()
    }
}

fn main() {
    log::set_logger(&CONSOLE_LOGGER).unwrap();
    log::set_max_level(LevelFilter::Info);
    let args: Vec<String> = env::args().collect();

    let mut options = Options {
        debug: false,
        url: String::new(),
        clear_cache: false,
    };

    for argument in &args[1..] {
        if argument == "--debug" {
            options.debug = true;
        } else if argument == "--clearCache" {
            options.clear_cache = true;
        } else if options.url.len() == 0 && !argument.starts_with('-') {
            options.url = String::from(argument);
        } else {
            panic!("Unknown argument {argument}")
        }
    }

    if options.debug {
        log::set_max_level(LevelFilter::Debug);
        log::debug!("Debug Mode enabled");
    } else {
        log::set_max_level(LevelFilter::Info);
    }

    Browser::new(options).run();
}
