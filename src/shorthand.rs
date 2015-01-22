
use std::cmp::min;
use css::{Value, Declaration};

static SHORTHAND: [&'static str; 2] = ["border", "margin"];

static MARGIN_PROPERTIES: [&'static str; 4] = ["margin-top", "margin-bottom", "margin-left", "margin-right"];
static MARGIN_ORDER: [[usize; 4]; 4] = [[0, 0, 0, 0], [0, 0, 1, 1], [0, 2, 1, 1], [0, 2, 3, 1]];


pub fn is_shorthand(name: &str) -> bool {
    SHORTHAND.contains(&name)
}

pub fn parse_shorthand(name: &str, values: Vec<Value>) -> Vec<Declaration> {
    match name {
        "border" => parse_border_shorthand(values),
        "margin" => parse_margin_shorthand(values),
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

fn parse_margin_shorthand(values: Vec<Value>) -> Vec<Declaration> {
    assert!(!values.is_empty());
    let idx = MARGIN_ORDER[min(4, values.len()) - 1];
    let mut declarations = Vec::new();
    for i in range(0, 4) {
        declarations.push(Declaration { name: MARGIN_PROPERTIES[i].to_string(), value: values[idx[i]].clone()});
    }
    return declarations;
}
