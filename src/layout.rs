use gtk::cairo::{Context, FontFace, FontSlant, FontWeight};

pub const DEFAULT_FONT_SIZE: f64 = 14.0;
pub const DEFAULT_FONT_FAMILY: &str = "Arial";
pub const DEFAULT_FONT_SLANT: FontSlant = FontSlant::Normal;
pub const DEFAULT_FONT_WEIGHT: FontWeight = FontWeight::Normal;
pub const H_STEP: f64 = 13.0;
pub const V_STEP: f64 = 18.0;

#[derive(Clone)]
pub enum Element {
    Tag(String),
    Text(String),
}

#[derive(Clone)]
pub struct HTMLContent {
    elements: Vec<Element>,
}

impl HTMLContent {
    pub fn new(elements: Vec<Element>) -> Self {
        HTMLContent { elements }
    }
}

#[derive(Clone)]
pub struct Position(pub f64, pub f64);

#[derive(Clone)]
pub struct DiscreteContent {
    pub content: String,
    pub position: Position,
    pub font_face: FontFace,
    pub font_size: f64,
}

pub struct Layout {
    font_size: f64,
    font_family: String,
    font_slant: FontSlant,
    font_weight: FontWeight,
    cursor_position: Position,
    window_width: f64,
}

impl Layout {
    pub fn from_html_content(
        context: &Context,
        content: &HTMLContent,
        window_width: f64,
    ) -> Vec<DiscreteContent> {
        let layout = Layout::new(DEFAULT_FONT_SIZE, DEFAULT_FONT_FAMILY, window_width);

        let display_list = layout.process(context, content);

        display_list
    }

    fn new(font_size: f64, font_family: &str, window_width: f64) -> Self {
        Layout {
            font_size,
            font_family: String::from(font_family),
            font_slant: DEFAULT_FONT_SLANT,
            font_weight: DEFAULT_FONT_WEIGHT,
            cursor_position: Position(H_STEP, V_STEP),
            window_width: window_width,
        }
    }

    pub fn get_current_position(&self) -> Position {
        self.cursor_position.clone()
    }

    fn process(mut self, context: &Context, content: &HTMLContent) -> Vec<DiscreteContent> {
        let current_font_face =
            FontFace::toy_create(&self.font_family, self.font_slant, self.font_weight)
                .expect("Failed to create font face");
        context.set_font_face(&current_font_face);
        context.set_font_size(self.font_size);

        let mut display_list: Vec<DiscreteContent> = Vec::new();

        for element in &content.elements {
            match element {
                Element::Text(text) => {
                    let words = text.split_ascii_whitespace().collect::<Vec<&str>>();
                    for word in words {
                        let discrete_content = self.word(context, word);
                        display_list.push(discrete_content);
                    }
                }
                Element::Tag(tag) => self.token(tag),
            }
        }

        display_list
    }

    fn get_space_width(&self, context: &Context) -> f64 {
        context.text_extents(" ").unwrap().x_advance()
    }

    fn token(&mut self, token: &String) {
        match token.as_str() {
            "i" | "em" => {
                self.font_slant = FontSlant::Italic;
            }
            "/i" | "/em" => {
                self.font_slant = FontSlant::Normal;
            }
            "b" | "strong" => {
                self.font_weight = FontWeight::Bold;
            }
            "/b" | "/strong" => {
                self.font_weight = FontWeight::Normal;
            }
            _ => {
                // Handle unknown tag
            }
        }
    }

    fn word(&mut self, context: &Context, word: &str) -> DiscreteContent {
        let word_width = context.text_extents(word).unwrap().x_advance();

        let word_height = context.font_extents().unwrap().height();
        let font_face = FontFace::toy_create(&self.font_family, self.font_slant, self.font_weight)
            .expect("Failed to create font face");

        let discrete_content = DiscreteContent {
            content: word.to_string(),
            position: self.cursor_position.clone(),
            font_face,
            font_size: self.font_size.clone(),
        };

        self.cursor_position.0 += word_width + self.get_space_width(context);

        if self.cursor_position.0 + word_width >= self.window_width - H_STEP {
            self.cursor_position.0 = H_STEP;
            self.cursor_position.1 += word_height * 1.25;
        }

        discrete_content
    }
}
