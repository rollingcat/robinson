
use std::cmp::min;
use css::{Value, Declaration};

static SHORTHAND: [&'static str; 4] = ["border", "border-width", "margin", "padding"];

static BORDER_WIDTH_PROPERTIES: [&'static str; 4] = ["border-top-width", "border-bottom-width", "border-left-width", "border-right-width"];
static MARGIN_PROPERTIES: [&'static str; 4] = ["margin-top", "margin-bottom", "margin-left", "margin-right"];
static PADDING_PROPERTIES: [&'static str; 4] = ["padding-top", "padding-bottom", "padding-left", "padding-right"];
static ORDER: [[usize; 4]; 4] = [[0, 0, 0, 0], [0, 0, 1, 1], [0, 2, 1, 1], [0, 2, 3, 1]];


pub fn is_shorthand(name: &str) -> bool {
    SHORTHAND.contains(&name)
}

pub fn parse_shorthand(name: &str, values: Vec<Value>) -> Vec<Declaration> {
    match name {
        "border" => parse_border_shorthand(values),
        "border-width" => parse_direction_shorthand(values, &BORDER_WIDTH_PROPERTIES),
        "margin" => parse_direction_shorthand(values, &MARGIN_PROPERTIES),
        "padding" => parse_direction_shorthand(values, &PADDING_PROPERTIES),
        _ => panic!("Not shorthand"),
    }
}

fn parse_border_shorthand(values: Vec<Value>) -> Vec<Declaration> {
    let mut declaration = Vec::new();
    for val in values.into_iter() {
        let decl_name = match val {
            Value::Length(_, _) => "border-width",
            Value::Keyword(_) => "border-style",
            Value::ColorValue(_) => "border-color",
        };
        declaration.push(Declaration { name: decl_name.to_string(), value: val });
    }
    return declaration;
}

fn parse_direction_shorthand(values: Vec<Value>, property: &[&str]) -> Vec<Declaration> {
    assert!(!values.is_empty());
    let idx = ORDER[min(4, values.len()) - 1];
    let mut declarations = Vec::new();
    for i in range(0, 4) {
        declarations.push(Declaration { name: property[i].to_string(), value: values[idx[i]].clone()});
    }
    return declarations;
}
