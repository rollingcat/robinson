//! A simple parser for a tiny subset of CSS.
//!
//! To support more CSS syntax, it would probably be easiest to replace this
//! hand-rolled parser with one based on a library or parser generator.

use std::ascii::OwnedAsciiExt; // for `into_ascii_lowercase`
use std::str::FromStr;
use std::num::FromStrRadix;
use color::{Color, ColorMap};
use shorthand;

// Data structures:

#[derive(Show)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Show)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

#[derive(Show)]
pub enum Selector {
    Simple(SimpleSelector),
    Descendant(DescendantSelector),
}

#[derive(Show)]
pub struct SimpleSelector {
    pub tag_name: Option<String>,
    pub id: Option<String>,
    pub class: Vec<String>,
}

pub type DescendantSelector = Vec<SimpleSelector>;

#[derive(Show)]
pub struct Declaration {
    pub name: String,
    pub value: Value,
}

#[derive(Show, Clone, PartialEq)]
pub enum Value {
    Keyword(String),
    Length(f32, Unit),
    ColorValue(Color),
}

#[derive(Show, Clone, PartialEq)]
pub enum Unit {
    Px,
    Em,
    Percent,
}

pub type Specificity = (usize, usize, usize);

static FONT_SIZE: f32 = 10.0f32;

impl Selector {
    pub fn specificity(&self) -> Specificity {
        // http://www.w3.org/TR/selectors/#specificity
        match *self {
            Selector::Simple(ref simple) => {
                let a = simple.id.iter().len();
                let b = simple.class.len();
                let c = simple.tag_name.iter().len();
                return (a, b, c);
            },
            Selector::Descendant(ref descendant) => {
                let mut specificity = (0, 0, 0);
                for i in descendant.iter() {
                    specificity.0 += i.id.iter().len();
                    specificity.1 += i.class.len();
                    specificity.2 += i.tag_name.iter().len();
                }
                return specificity;
            }
        }
    }
}

impl Value {
    /// Return the size of a length in px, or zero for non-lengths.
    pub fn to_px(&self) -> f32 {
        match *self {
            Value::Length(f, Unit::Px) => f,
            Value::Length(f, Unit::Em) => f * FONT_SIZE,
            _ => 0.0
        }
    }
}

/// Parse a whole CSS stylesheet.
pub fn parse(source: String) -> Stylesheet {
    let mut parser = Parser { pos: 0, input: source, color_map: ColorMap::new() };
    Stylesheet { rules: parser.parse_rules() }
}

pub fn parse_inline_style(source: String) -> Vec<Declaration> {
    let mut parser = Parser { pos: 0, input: source, color_map: ColorMap::new() };
    parser.parse_declarations()
}

struct Parser {
    pos: usize,
    input: String,
    color_map: ColorMap,
}

impl Parser {
    /// Parse a list of rule sets, separated by optional whitespace.
    fn parse_rules(&mut self) -> Vec<Rule> {
        let mut rules = Vec::new();
        loop {
            self.consume_whitespace();
            if self.eof() { break }
            rules.push(self.parse_rule());
        }
        return rules;
    }

    /// Parse a rule set: `<selectors> { <declarations> }`.
    fn parse_rule(&mut self) -> Rule {
        Rule {
            selectors: self.parse_all_selectors(),
            declarations: self.parse_declarations(),
        }
    }

    /// Parse a comma-separated list of selectors.
    fn parse_selectors(&mut self) -> Vec<Selector> {
        let mut selectors = Vec::new();
        loop {
            selectors.push(Selector::Simple(self.parse_simple_selector()));
            self.consume_whitespace();
            match self.next_char() {
                ',' => { self.consume_char(); self.consume_whitespace(); }
                '{' => break,
                c   => panic!("Unexpected character {} in selector list", c)
            }
        }
        // Return selectors with highest specificity first, for use in matching.
        selectors.sort_by(|a,b| b.specificity().cmp(&a.specificity()));
        return selectors;
    }

    fn parse_all_selectors(&mut self) -> Vec<Selector> {
        let mut selectors: Vec<Selector> = Vec::new();
        let mut simple;

        while self.next_char() != '{' {
            let mut descendant: Vec<SimpleSelector> = Vec::new();
            loop {
                simple = self.parse_simple_selector();
                self.consume_whitespace();
                match self.next_char() {
                    ',' => { self.consume_char(); self.consume_whitespace(); break; },
                    '{' => break,
                    c => descendant.push(simple)
                }
            }
            if descendant.is_empty() {
                selectors.push(Selector::Simple(simple));
            } else {
                descendant.push(simple);
                selectors.push(Selector::Descendant(descendant));
            }
        }
        // Return selectors with highest specificity first, for use in matching.
        selectors.sort_by(|a,b| b.specificity().cmp(&a.specificity()));
        return selectors;
    }

    /// Parse one simple selector, e.g.: `type#id.class1.class2.class3`
    fn parse_simple_selector(&mut self) -> SimpleSelector {
        let mut selector = SimpleSelector { tag_name: None, id: None, class: Vec::new() };
        while !self.eof() {
            match self.next_char() {
                '#' => {
                    self.consume_char();
                    selector.id = Some(self.parse_identifier());
                }
                '.' => {
                    self.consume_char();
                    selector.class.push(self.parse_identifier());
                }
                '*' => {
                    // universal selector
                    self.consume_char();
                }
                c if valid_identifier_char(c) => {
                    selector.tag_name = Some(self.parse_identifier().into_ascii_lowercase());
                }
                _ => break
            }
        }
        return selector;
    }

    /// Parse a list of declarations enclosed in `{ ... }`.
    fn parse_declarations(&mut self) -> Vec<Declaration> {
        assert!(self.consume_char() == '{');
        let mut declarations = Vec::new();
        loop {
            self.consume_whitespace();
            if self.next_char() == '}' {
                self.consume_char();
                break;
            }
            for decl in self.parse_declaration().into_iter() {
                declarations.push(decl);
            }
        }
        return declarations;
    }

    /// Parse one `<property>: <value>;` declaration.
    fn parse_declaration(&mut self) -> Vec<Declaration> {
        self.consume_comment();

        let property_name = self.parse_identifier();
        self.consume_whitespace();
        assert!(self.consume_char() == ':');
        self.consume_whitespace();

        let mut declarations = Vec::new();
        if shorthand::is_shorthand(property_name.as_slice()) {
            declarations = shorthand::parse_shorthand(property_name.as_slice(), self.parse_values());
        } else {
            let value = self.parse_value();
            self.consume_whitespace();
            declarations.push(Declaration { name: property_name, value: value });
        }
        assert!(self.consume_char() == ';');
        self.consume_comment();
        declarations
    }

    fn parse_values(&mut self) -> Vec<Value> {
        let mut values = Vec::new();
        while self.next_char() != ';' {
            values.push(self.parse_value());
            self.consume_whitespace();
        }
        values
    }

    // Methods for parsing values:

    fn parse_value(&mut self) -> Value {
        match self.next_char() {
            '0'...'9' | '.' => self.parse_length(),
            '#' => self.parse_color(),
            _ => {
                let value = self.parse_identifier();
                match self.color_map.get_color(value.as_slice()) {
                    Some(color) => Value::ColorValue(*color),
                    None => Value::Keyword(value),
                }
            }
        }
    }

    fn parse_value_to_string(&mut self) -> String {
        self.consume_while(|c| c != ';')
    }

    fn parse_length(&mut self) -> Value {
        Value::Length(self.parse_float(), self.parse_unit())
    }

    fn parse_float(&mut self) -> f32 {
        let s = self.consume_while(|c| match c {
            '0'...'9' | '.' => true,
            _ => false
        });
        let f: Option<f32> = FromStr::from_str(&*s);
        f.unwrap()
    }

    fn parse_unit(&mut self) -> Unit {
        match &*self.parse_identifier().into_ascii_lowercase() {
            "px" | "" => Unit::Px,
            "em" => Unit::Em,
            "%" => Unit::Percent,
            _ => panic!("unrecognized unit")
        }
    }

    fn parse_color(&mut self) -> Value {
        assert!(self.consume_char() == '#');
        let mut hex = self.consume_while(|c| c != ';');
        Value::ColorValue(convert_hex_to_color(&mut hex))
    }

    /// Parse two hexadecimal digits.
    fn parse_hex_pair(&mut self) -> u8 {
        let s = self.input.slice(self.pos, self.pos + 2);
        self.pos = self.pos + 2;
        FromStrRadix::from_str_radix(s, 0x10).unwrap()
    }

    /// Parse a property name or keyword.
    fn parse_identifier(&mut self) -> String {
        self.consume_while(valid_identifier_char)
    }

    /// Consume and discard zero or more whitespace characters.
    fn consume_whitespace(&mut self) {
        self.consume_while(|c| c.is_whitespace());
    }

    /// Consume characters until `test` returns false.
    fn consume_while<F: Fn(char) -> bool>(&mut self, test: F) -> String {
        let mut result = String::new();
        while !self.eof() && test(self.next_char()) {
            result.push(self.consume_char());
        }
        return result;
    }

    /// Return the current character, and advance self.pos to the next character.
    fn consume_char(&mut self) -> char {
        let range = self.input.char_range_at(self.pos);
        self.pos = range.next;
        return range.ch;
    }

    /// Read the current character without consuming it.
    fn next_char(&self) -> char {
        self.input.char_at(self.pos)
    }

    /// Return true if all input is consumed.
    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    /// Does the current input start with the given string?
    fn starts_with(&self, s: &str) -> bool {
        self.input.slice_from(self.pos).starts_with(s)
    }

    fn consume_comment(&mut self) {
        self.consume_whitespace();
        while self.starts_with("/*") {
            assert!(self.consume_char() == '/');
            assert!(self.consume_char() == '*');
            self.consume_while(|c| c != '/');
            assert!(self.consume_char() == '/');
            self.consume_whitespace();
        }
    }
}

fn valid_identifier_char(c: char) -> bool {
    match c {
        'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '_' | '%' => true, // TODO: Include U+00A0 and higher.
        _ => false,
    }
}

fn convert_hex_to_color(input: &mut String) -> Color {
    if (input.len() / 3) == 1 {
        for i in range(0, 3).rev() {
            let c = input.char_at(i);
            input.insert(i + 1, c);
        }
    }
    Color {
        r: FromStrRadix::from_str_radix(input.slice(0, 2), 0x10).unwrap(),
        g: FromStrRadix::from_str_radix(input.slice(2, 4), 0x10).unwrap(),
        b: FromStrRadix::from_str_radix(input.slice(4, 6), 0x10).unwrap(),
        a: 255,
    }
}
