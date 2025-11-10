use crate::logger::CONSOLE_LOGGER;
use crate::request::{Request, RequestOptions};
use crate::uri::Scheme;
use crate::uri::URI;

use core::panic;
use gtk::gio::ApplicationCommandLine;
use gtk::glib::ExitCode;
use gtk::{cairo, gio, pango, Application, DrawingArea};
use gtk::{prelude::*, ApplicationWindow};
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
    app: Application,
    request: Request,
    content: String,
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

        let app = Application::new(Some(APP_ID), Default::default());

        log::debug!("Application initialized");

        Self {
            options,
            url: String::from(""),
            app,
            request: requester,
            content: String::new(),
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

    fn build_drawing_area() -> DrawingArea {
        log::debug!("Creating Drawing Area");

        let drawing_area = DrawingArea::new();
        drawing_area.set_content_width(WIDTH);
        drawing_area.set_content_height(HEIGHT);

        drawing_area
    }

    fn build_window(app: &Application) -> ApplicationWindow {
        log::debug!("Building UI");
        let window = ApplicationWindow::new(app);

        // Create a window and set the title
        window.set_title(Some("Bored Browser"));
        window.set_default_width(WIDTH);
        window.set_default_height(HEIGHT);

        window
    }

    fn draw_to(area: &DrawingArea, display_list: Vec<DiscreteContent>, context: &cairo::Context) {
        for content in display_list {
            // Create a Pango layout for the parsed content
            let layout = area.create_pango_layout(Some(&content.content.to_string()));

            // Set font
            let font_desc = pango::FontDescription::from_string("Sans 14");
            layout.set_font_description(Some(&font_desc));

            // Set text color to black
            context.set_source_rgba(0.0, 0.0, 0.0, 1.0);

            // Position the text with some margin
            context.move_to(content.position.0, content.position.1);

            // Draw the layout
            pangocairo::functions::show_layout(context, &layout);
        }
    }

    fn handle_command_line(app: &Application, command_line: &ApplicationCommandLine) -> ExitCode {
        let args = command_line.arguments();

        match args.len() {
            1 => {
                println!("No URL provided");
                return ExitCode::FAILURE;
            }
            _ => {
                app.activate();
                return ExitCode::SUCCESS;
            }
        }
    }

    pub fn run(&mut self, url: String) {
        let mut flags = gio::ApplicationFlags::empty();
        flags.insert(gio::ApplicationFlags::HANDLES_COMMAND_LINE);
        self.app.set_flags(flags);

        self.app.connect_command_line(Self::handle_command_line);

        self.load(url);

        log::debug!("Response Loaded");

        let content = self.content.to_string();
        self.app.connect_activate(move |app| {
            log::debug!("Connection activated");
            let window = Self::build_window(&app);

            let drawing_area = Self::build_drawing_area();

            let content_clone = content.clone();

            drawing_area.set_draw_func(move |area, cr, width, height| {
                // Set background color to white
                cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                cr.paint().unwrap();

                let display_list = Self::layout(&content_clone, width);

                Self::draw_to(area, display_list, cr);
            });

            window.set_child(Some(&drawing_area));
            // Present window
            window.present();
        });

        self.app.run();
    }
}

fn main() {
    log::set_logger(&CONSOLE_LOGGER).unwrap();
    log::set_max_level(LevelFilter::Info);
    let args: Vec<String> = env::args().collect();

    let mut options = Options {
        debug: false,
        clear_cache: false,
    };

    for argument in &args[1..] {
        if argument == "--debug" {
            options.debug = true;
        } else if argument == "--clearCache" {
            options.clear_cache = true;
        }
    }

    if options.debug {
        log::set_max_level(LevelFilter::Debug);
        log::debug!("Debug Mode enabled");
    } else {
        log::set_max_level(LevelFilter::Info);
    }

    Browser::new(options).run(args[1].clone());
}
