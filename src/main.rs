use crate::logger::CONSOLE_LOGGER;
use crate::request::{Request, RequestOptions};
use crate::uri::Scheme;
use crate::uri::URI;

use core::panic;
use gtk::gio::{Application, ApplicationCommandLine};
use gtk::glib::ExitCode;

// use gio::prelude::*;
use gtk::DrawingArea;

use gtk::cairo::{Context, FontSlant, FontWeight};
use gtk::gio;
use gtk::prelude::*;
use log::LevelFilter;
use pangocairo;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;

pub mod cache;
pub mod logger;
pub mod request;
pub mod uri;

const APP_ID: &str = "com.greghamel.bored-browser";

const DEBUG_STR: &str = "--debug";
const CLEAR_CACHE_STR: &str = "--clear-cache";

struct Options {
    debug: bool,
    clear_cache: bool,
}
#[derive(Clone)]
struct DiscreteContent {
    content: String,
    position: (f64, f64),
}

struct Browser {
    options: Options,
    url: String,
    request: Request,
    content: String,
    scroll_position: i32,
}

const H_STEP: f64 = 13.0;
const V_STEP: f64 = 18.0;

const WIDTH: i32 = 600;
const HEIGHT: i32 = 600;

impl Browser {
    pub fn new(options: Options) -> Self {
        let cache = cache::Cache::initialize(options.clear_cache);

        let requester = Request::init(RequestOptions { cache });
        log::debug!("Request initialized");

        log::debug!("Application initialized");

        Self {
            options,
            url: String::from(""),
            request: requester,
            content: String::new(),
            scroll_position: 100,
        }
    }

    fn transform(&mut self, data: &str) -> String {
        let lt_re = Regex::new(r"<").unwrap();
        let gt_re = Regex::new(r">").unwrap();

        let no_lt = String::from(lt_re.replace_all(data, "&lt;"));

        String::from(gt_re.replace_all(&no_lt.as_str(), "&gt;"))
    }

    fn lex(&mut self, source: &str, only_body: bool) -> String {
        let mut in_angle = false;
        let mut in_body = false;
        let mut result = String::new();

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
                            result.push_str(&possible_entity);
                            possible_entity = String::new();
                            continue;
                        }

                        possible_entity += &character.to_string();

                        if character == ';' {
                            if html_entities.contains_key(&possible_entity.as_str()) {
                                let string_value =
                                    html_entities.get(&possible_entity.as_str()).unwrap_or(&"");
                                result.push_str(string_value);
                            } else {
                                result.push_str(&possible_entity);
                            }

                            possible_entity = String::new();
                        }
                    }

                    continue;
                }

                result.push(character);
            } else if in_angle {
                current_tag += &character.to_string();
            }
        }

        if possible_entity.len() > 0 {
            // If buffer still full, dump its content
            result.push_str(&possible_entity);
        }

        result
    }

    fn show(&mut self, source: &str, only_body: bool) {
        self.content = self.lex(source, only_body);
    }

    fn load(&mut self, url: String) {
        let uri = URI::parse(&url);

        match uri.scheme {
            Scheme::HTTPS | Scheme::HTTP => {
                log::debug!("Loading HTTP/HTTPS URL");
                let response = self.request.send(&uri).expect("Couldn't parse response...");

                log::debug!("Response received");

                if uri.flags.contains_key(&String::from("view-source")) {
                    log::debug!("View-source flag found");
                    let transformed_response = self.transform(&response.data.as_str());
                    self.show(&transformed_response, false)
                } else {
                    log::debug!("Rendering response");
                    self.show(&response.data, true)
                }
            }
            Scheme::Data => {
                log::debug!("Loading Data URL");

                // _ is the content_type
                let (_, path_data) = uri.path.split_once(',').unwrap_or((&uri.path, ""));

                // Writing end-of-file.
                let data = String::new() + path_data + "\r\n";
                self.show(&data, false)
            }
            Scheme::File => {
                log::debug!("Loading File URL");

                let data = fs::read_to_string(&uri.path).expect("File not found...");
                self.show(&data, false)
            }
            Scheme::VIEWSOURCE => panic!("Unexpected view-source scheme provided to browser."),
        }
    }

    fn layout(text: &str, window_width: i32) -> Vec<DiscreteContent> {
        log::debug!("Layout function called");
        let mut display_list: Vec<DiscreteContent> = Vec::new();
        let mut cursor_x = H_STEP;
        let mut cursor_y = V_STEP;

        let characters = text.chars();

        for content in characters {
            let discrete_content = DiscreteContent {
                content: content.to_string(),
                position: (cursor_x, cursor_y),
            };

            display_list.push(discrete_content);

            match content {
                '\n' => {
                    cursor_x = H_STEP;
                    cursor_y += V_STEP;
                }
                _ => {
                    cursor_x += H_STEP;
                }
            }

            if cursor_x as i32 >= window_width {
                cursor_x = H_STEP;
                cursor_y += V_STEP;
            }
        }

        display_list
    }

    fn render(&self, application: &gtk::Application) {
        log::debug!("Rendering");

        let content_clone = self.content.to_string();
        let scroll_position_clone = self.scroll_position.clone();

        drawable(application, WIDTH, HEIGHT, move |_, cr, width, height| {
            // Set background color to white
            cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);

            match cr.paint() {
                Ok(_) => (),
                Err(err) => log::error!("Error painting: {}", err),
            }

            log::debug!("Painted a white background");

            let display_list = Self::layout(&content_clone, width);

            Self::draw_to(display_list, scroll_position_clone, cr);
        });
    }

    fn draw_to(display_list: Vec<DiscreteContent>, scroll_position: i32, context: &Context) {
        log::debug!("Drawing to area");
        context.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
        context.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        context.set_font_size(12.0);

        for content in display_list {
            context.move_to(
                content.position.0,
                content.position.1 - scroll_position as f64,
            );
            let text_rendering = context.show_text(&content.content.to_string());

            match text_rendering {
                Ok(_) => (),
                Err(err) => log::error!("Error rendering text: {}", err),
            }
        }
    }

    pub fn run(&mut self, app: &gtk::Application, url: String) {
        self.load(url);

        log::debug!("Response Loaded");

        self.render(app);
    }
}

fn main() {
    log::set_logger(&CONSOLE_LOGGER).unwrap();
    log::set_max_level(LevelFilter::Debug);

    let app = gtk::Application::new(Some(APP_ID), Default::default());

    let flags = gio::ApplicationFlags::empty();
    // flags.insert(gio::ApplicationFlags::HANDLES_COMMAND_LINE);

    app.set_flags(flags);

    app.connect_activate(|app| {
        // app.connect_command_line(|app, command_line| {
        let options = Options {
            debug: true,
            clear_cache: false,
        };

        //     let args = command_line.arguments();

        //     for arg in args.iter().skip(1) {
        //         let arg_str = arg.to_string_lossy();
        //         if arg_str == DEBUG_STR {
        //             options.debug = true;
        //         } else if arg_str == CLEAR_CACHE_STR {
        //             options.clear_cache = true;
        //         }
        //     }

        //     if options.debug {
        //         log::set_max_level(LevelFilter::Debug);
        //         log::debug!("Debug Mode enabled");
        //     } else {
        //         log::set_max_level(LevelFilter::Info);
        //     }

        Browser::new(options).run(app, String::from("https://browser.engineering/http.html"));

        // ExitCode::SUCCESS
        // });
    });

    app.run();
}

pub fn drawable<F>(application: &gtk::Application, width: i32, height: i32, draw_fn: F)
where
    F: Fn(&DrawingArea, &Context, i32, i32) + 'static,
{
    let window = gtk::ApplicationWindow::new(application);
    let drawing_area = Box::new(DrawingArea::new)();

    drawing_area.set_draw_func(draw_fn);

    window.set_default_size(width, height);

    window.set_child(Some(&drawing_area));
    window.present();
}
