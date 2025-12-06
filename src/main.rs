use crate::logger::CONSOLE_LOGGER;
use crate::request::{Request, RequestOptions};
use crate::uri::Scheme;
use crate::uri::URI;

use core::panic;
use std::sync::OnceLock;
use std::time::Duration;

use gtk::cairo::{Context, FontFace, FontSlant, FontWeight};
use gtk::prelude::*;
use gtk::{gdk, glib};
use log::LevelFilter;
use regex::Regex;
use relm4::abstractions::DrawHandler;
use relm4::{gtk, tokio, Component, ComponentParts, ComponentSender, RelmApp};
use std::collections::HashMap;
use std::fs;

pub mod cache;
pub mod logger;
pub mod request;
pub mod uri;

const APP_ID: &str = "com.greghamel.bored-browser";

#[derive(Debug)]
struct Options {
    debug: bool,
    clear_cache: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            debug: false,
            clear_cache: false,
        }
    }
}

const DEFAULT_FONT_SIZE: f64 = 14.0;
const DEFAULT_FONT_FAMILY: &str = "Arial";
const DEFAULT_FONT_SLANT: FontSlant = FontSlant::Normal;
const DEFAULT_FONT_WEIGHT: FontWeight = FontWeight::Normal;

fn set_font_defaults(context: &Context) {
    context.select_font_face(DEFAULT_FONT_FAMILY, DEFAULT_FONT_SLANT, DEFAULT_FONT_WEIGHT);
    context.set_font_size(DEFAULT_FONT_SIZE);
}

#[derive(Clone)]
enum Element {
    Tag(String),
    Text(String),
}

#[derive(Clone)]
struct Position(f64, f64);

#[derive(Clone)]
struct DiscreteContent {
    content: String,
    position: Position,
    font_face: FontFace,
}

#[derive(Clone)]
struct HTMLContent {
    elements: Vec<Element>,
}

struct Browser {
    width: f64,
    height: f64,
    url: String,
    request: Request,
    content: HTMLContent,
    display_list: Vec<DiscreteContent>,
    scroll_position: i32,
    draw_handler: DrawHandler,
}

const H_STEP: f64 = 13.0;
const V_STEP: f64 = 18.0;

const WIDTH: f64 = 600.0;
const HEIGHT: f64 = 600.0;

pub(crate) static APPLICATION_OPTS: OnceLock<Options> = OnceLock::new();
pub(crate) static REQUEST_URL: OnceLock<String> = OnceLock::new();

impl Browser {
    pub fn new() -> Self {
        let cache = cache::Cache::initialize();

        let requester = Request::init(RequestOptions { cache });
        log::debug!("Request initialized");

        log::debug!("Application initialized");

        Self {
            height: HEIGHT,
            width: WIDTH,
            url: String::new(),
            display_list: Vec::new(),
            request: requester,
            content: HTMLContent {
                elements: Vec::new(),
            },
            scroll_position: 0,
            draw_handler: DrawHandler::new(),
        }
    }

    fn transform(data: &str) -> String {
        let lt_re = Regex::new(r"<").unwrap();
        let gt_re = Regex::new(r">").unwrap();

        let no_lt = String::from(lt_re.replace_all(data, "&lt;"));

        String::from(gt_re.replace_all(&no_lt.as_str(), "&gt;"))
    }

    fn show(&mut self, source: &str) {
        self.content = lex(source);
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
                    let transformed_response = Browser::transform(&response.data.as_str());
                    self.show(&transformed_response)
                } else {
                    log::debug!("Rendering response");
                    self.show(&response.data)
                }
            }
            Scheme::Data => {
                log::debug!("Loading Data URL");

                // _ is the content_type
                let (_, path_data) = uri.path.split_once(',').unwrap_or((&uri.path, ""));

                // Writing end-of-file.
                let data = String::new() + path_data + "\r\n";
                self.show(&data)
            }
            Scheme::File => {
                log::debug!("Loading File URL");

                let data = fs::read_to_string(&uri.path).expect("File not found...");
                self.show(&data)
            }
            Scheme::VIEWSOURCE => panic!("Unexpected view-source scheme provided to browser."),
        }
    }

    pub fn run(&mut self) {
        // self.load(REQUEST_URL.get().expect("REQUEST_URL must be set"));
        self.load(String::from("https://browser.engineering/http.html"));

        log::debug!("Response Loaded");
    }
}

#[derive(Debug)]
enum AppMsg {
    KBScrollDown,
    KBScrollUp,
    MouseScroll((f64, f64)),
    ContentParsed,
    Resize,
}

#[derive(Debug)]
struct UpdateRenderMsg;

#[relm4::component]
impl Component for Browser {
    type Input = AppMsg;
    type Output = ();
    type Init = ();
    type CommandOutput = UpdateRenderMsg;

    view! {
      gtk::Window {
        set_default_size: (600, 300),
        add_controller = gtk::EventControllerKey {
          connect_key_pressed[sender] => move |_, key, _, _| {
              log::debug!("Key pressed");
              match key {
                  gdk::Key::Escape => {
                      log::info!("Esc Key registered, exiting...");
                      std::process::exit(0);
                  }
                  gdk::Key::Down => {
                      log::debug!("Down key pressed");
                      sender.input(AppMsg::KBScrollDown);
                  }
                  gdk::Key::Up => {
                      log::debug!("Up key pressed");
                      sender.input(AppMsg::KBScrollUp);
                  }
                  _ => (),
              }
              glib::Propagation::Proceed
          },
        },

        add_controller = gtk::EventControllerScroll {
            set_flags: gtk::EventControllerScrollFlags::VERTICAL,

            connect_scroll[sender] => move |_, dx, dy| {
                sender.input(AppMsg::MouseScroll((dx, dy)));
                glib::Propagation::Proceed
            }
        },

        gtk::Box {
          set_orientation: gtk::Orientation::Vertical,
          // set_margin_all: 10,
          // set_spacing: 10,
          set_hexpand: true,

          #[local_ref]
          area -> gtk::DrawingArea {
            set_vexpand: true,
            set_hexpand: true,

            add_controller = gtk::GestureClick {
            },
            connect_resize[sender] => move |_, _, _| {
                log::debug!("Resize event");
                sender.input(AppMsg::Resize);
            }
          },
        }
      }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let mut model = Browser::new();

        let area = model.draw_handler.drawing_area();
        let widgets = view_output!();

        sender.command(|out, shutdown| {
            shutdown
                .register(async move {
                    loop {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        out.send(UpdateRenderMsg).unwrap();
                    }
                })
                .drop_on_shutdown()
        });

        model.run();

        sender.input(AppMsg::ContentParsed);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: AppMsg, _sender: ComponentSender<Self>, _root: &Self::Root) {
        let cx = self.draw_handler.get_context();

        match message {
            AppMsg::KBScrollDown => {
                self.scroll_position = self.scroll_position.wrapping_add(V_STEP as i32);
            }
            AppMsg::KBScrollUp => {
                if self.scroll_position > 0 {
                    self.scroll_position = self.scroll_position.wrapping_sub(V_STEP as i32);
                }
            }
            AppMsg::ContentParsed => {
                self.content = self.content.clone();
                self.display_list = layout(&cx, &self.content, self.width);
            }
            AppMsg::Resize => {
                self.width = self.draw_handler.width() as f64;
                self.height = self.draw_handler.height() as f64;
                self.display_list = layout(&cx, &self.content, self.width);
            }
            AppMsg::MouseScroll((_, dy)) => {
                let scroll_distance = 10.0 * dy;

                if self.scroll_position + scroll_distance as i32 > 0
                    && self.display_list.last().unwrap().position.1
                        > self.scroll_position as f64 + scroll_distance + self.height
                {
                    self.scroll_position =
                        self.scroll_position.wrapping_add(scroll_distance as i32);
                } else if (self.scroll_position + scroll_distance as i32) < 0 {
                    self.scroll_position = 0;
                } else if self.scroll_position + scroll_distance as i32
                    > self.display_list.last().unwrap().position.1 as i32
                {
                    self.scroll_position =
                        self.display_list.last().unwrap().position.1 as i32 - self.height as i32;
                }
            }
        }

        render(
            &cx,
            &self.display_list,
            self.scroll_position,
            self.height,
            self.width,
        );
    }

    fn update_cmd(&mut self, _: UpdateRenderMsg, _: ComponentSender<Self>, _root: &Self::Root) {
        let cx = self.draw_handler.get_context();
        render(
            &cx,
            &self.display_list,
            self.scroll_position,
            self.height,
            self.width,
        );
    }
}

fn lex(source: &str) -> HTMLContent {
    let mut in_tag = false;
    let mut result = Vec::new();

    let html_entities = HashMap::from([("&lt;", "<"), ("&gt;", ">")]);

    let mut buffer = String::new();

    for character in source.chars() {
        if character == '<' {
            in_tag = true;

            if buffer.len() > 0 {
                if let Some(entity) = html_entities.get(&buffer.as_str()) {
                    result.push(Element::Text(entity.to_string()));
                } else {
                    result.push(Element::Text(buffer));
                }
                buffer = String::new();
            }
        } else if character == '>' {
            result.push(Element::Tag(buffer));

            buffer = String::new();
            in_tag = false;
        } else if !in_tag {
            if character == ';' && buffer.len() > 0 {
                let potential_entity = buffer.clone() + &character.to_string().as_str();
                if html_entities.contains_key(&potential_entity.as_str()) {
                    let string_value = html_entities.get(&potential_entity.as_str()).unwrap_or(&"");
                    result.push(Element::Text(string_value.to_string()));
                } else {
                    result.push(Element::Text(buffer));
                }

                buffer = String::new();

                continue;
            }

            buffer += &character.to_string();
        } else {
            buffer += &character.to_string();
        }
    }

    if buffer.len() > 0 && !in_tag {
        // If buffer still has text content, dump its content
        result.push(Element::Text(buffer));
    }

    HTMLContent { elements: result }
}

fn layout(context: &Context, content: &HTMLContent, window_width: f64) -> Vec<DiscreteContent> {
    set_font_defaults(context);

    let word_height = context.font_extents().unwrap().height();
    let mut display_list: Vec<DiscreteContent> = Vec::new();
    let mut cursor_x = H_STEP;
    let mut cursor_y = word_height;

    let space_width = context.text_extents(" ").unwrap().x_advance();

    let mut font_slant = FontSlant::Normal;
    let mut font_weight = FontWeight::Normal;

    for element in &content.elements {
        match element {
            Element::Text(text) => {
                let words = text.split_ascii_whitespace().collect::<Vec<&str>>();
                for word in words {
                    let word_width = context.text_extents(word).unwrap().x_advance();

                    let font_face =
                        FontFace::toy_create(DEFAULT_FONT_FAMILY, font_slant, font_weight)
                            .expect("Failed to create font face");

                    let discrete_content = DiscreteContent {
                        content: word.to_string(),
                        position: Position(cursor_x, cursor_y),
                        font_face,
                    };

                    display_list.push(discrete_content);

                    match word {
                        _ => {
                            cursor_x += word_width + space_width;
                        }
                    }

                    if cursor_x + word_width >= window_width - H_STEP {
                        cursor_x = H_STEP;
                        cursor_y += word_height * 1.25;
                    }
                }
            }
            Element::Tag(tag) => {
                match tag.as_str() {
                    "i" | "em" => {
                        font_slant = FontSlant::Italic;
                    }
                    "/i" | "/em" => {
                        font_slant = FontSlant::Normal;
                    }
                    "b" | "strong" => {
                        font_weight = FontWeight::Bold;
                    }
                    "/b" | "/strong" => {
                        font_weight = FontWeight::Normal;
                    }
                    _ => {
                        // Handle unknown tag
                    }
                }
            }
        }
    }

    display_list
}

fn draw_content(
    display_list: &Vec<DiscreteContent>,
    scroll_position: i32,
    context: &Context,
    window_height: f64,
) {
    set_font_defaults(context);
    context.set_source_rgba(0.0, 0.0, 0.0, 1.0);

    for content in display_list {
        // Don't draw content that is off-screen
        if content.position.1 < scroll_position as f64
            || content.position.1 > scroll_position as f64 + window_height
        {
            continue;
        }

        context.set_font_face(&content.font_face);

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

fn draw_scrollbar(
    cx: &Context,
    height: f64,
    width: f64,
    content_height: f64,
    scroll_position: f64,
) {
    let scrollbar_height = height * height / content_height;
    let scrollbar_y = scroll_position * height / content_height;
    let scrollbar_width = 10.0;

    cx.set_source_rgb(0.5, 0.5, 0.5);
    cx.rectangle(
        width - scrollbar_width,
        scrollbar_y,
        scrollbar_width,
        scrollbar_height,
    );
    cx.fill().expect("Couldn't fill scrollbar");
}

fn get_content_height(display_list: &Vec<DiscreteContent>) -> f64 {
    return match display_list.last() {
        Some(content) => content.position.1,
        None => 0.0,
    };
}

fn render(
    cx: &Context,
    display_list: &Vec<DiscreteContent>,
    scroll_position: i32,
    window_height: f64,
    window_width: f64,
) {
    let scroll_position_clone = scroll_position.clone();

    cx.set_source_rgba(1.0, 1.0, 1.0, 1.0);

    match cx.paint() {
        Ok(_) => (),
        Err(err) => log::error!("Error painting: {}", err),
    }

    draw_content(display_list, scroll_position_clone, cx, window_height);
    draw_scrollbar(
        cx,
        window_height,
        window_width,
        get_content_height(display_list),
        scroll_position as f64,
    );
}

fn main() {
    log::set_logger(&CONSOLE_LOGGER).unwrap();
    log::set_max_level(LevelFilter::Debug);
    let application_opts = Options::default();

    APPLICATION_OPTS
        .set(application_opts)
        .expect("Failed to set application options, already initialized");

    let app = RelmApp::new(APP_ID);
    app.run::<Browser>(());
}
