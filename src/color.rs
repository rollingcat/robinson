
use std::collections::HashMap;

#[derive(Show, Clone, PartialEq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Copy for Color {}
impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color { r: r, g: g, b: b, a: a, }
    }
}

pub struct ColorMap {
    map: HashMap<String, Color>,
}

impl ColorMap {
    pub fn new() -> ColorMap {
        let mut map = HashMap::new();

        map.insert("black".to_string(), Color::new(0, 0, 0, 255));
        map.insert("white".to_string(), Color::new(255, 255, 255, 255));
        map.insert("red".to_string(), Color::new(255, 0, 0, 255));
        map.insert("green".to_string(), Color::new(0, 255, 0, 255));
        map.insert("blue".to_string(), Color::new(0, 0, 255, 255));

        ColorMap {
            map: map,
        }
    }

    pub fn get_color(&self, string: &str) -> Option<&Color> {
        self.map.get(string)
    }
}
