//! Code for applying CSS styles to the DOM.
//!
//! This is not very interesting at the moment.  It will get much more
//! complicated if I add support for compound selectors.

use dom::{Node, NodeType, ElementData};
use css::{Stylesheet, Rule, Selector, SimpleSelector, Value, Specificity};
use std::collections::HashMap;
use std::rc::Rc;
use std::rc::Weak;

use dom;
use css;
use color::{Color};

/// Map from CSS property names to values.
pub type PropertyMap =  HashMap<String, Value>;

/// A node with associated style data.
pub struct StyledNode<'a> {
    pub node: Rc<Node>,
    pub specified_values: PropertyMap,
    pub children: Vec<StyledNode<'a>>,
}

#[derive(PartialEq)]
pub enum Display {
    Inline,
    Block,
    None,
}

#[derive(PartialEq)]
pub enum Float {
    FloatLeft,
    FloatRight,
}

#[derive(PartialEq)]
pub enum Clear {
    ClearLeft,
    ClearRight,
    ClearBoth,
}

static NONE_DISPLAY: [&'static str; 4] = ["head", "meta", "title", "style"];
static DEFAULT_BLOCK: [&'static str; 11] =
["address", "blockquote", "dd", "div", "dl", "form", "p", "ul", "h1", "html", "body"];
static DEFAULT_INHERIT: [&'static str; 3] = ["color", "font-size", "line-height"];

impl<'a> StyledNode<'a> {
    /// Return the specified value of a property if it exists, otherwise `None`.
    pub fn value(&self, name: &str) -> Option<Value> {
        self.specified_values.get(name).map(|v| v.clone())
    }

    /// Return the specified value of property `name`, or property `fallback_name` if that doesn't
    /// exist. or value `default` if neither does.
    pub fn lookup(&self, name: &str, fallback_name: &str, default: &Value) -> Value {
        self.value(name).unwrap_or_else(|| self.value(fallback_name)
                        .unwrap_or_else(|| default.clone()))
    }

    /// The value of the `display` property (defaults to inline).
    pub fn display(&self) -> Display {
        match self.value("display") {
            Some(Value::Keyword(s)) => match s.as_slice() {
                "block" => Display::Block,
                "none" => Display::None,
                _ => Display::Inline
            },
            _ => self.default_display()
        }
    }

    fn default_display(&self) -> Display {
        if DEFAULT_BLOCK.contains(&self.tag_name().as_slice()) {
            return Display::Block;
        }
        return Display::Inline;
    }

    pub fn float_value(&self) -> Option<Float> {
        match self.value("float") {
            Some(Value::Keyword(s)) => match s.as_slice() {
                "left" => Some(Float::FloatLeft),
                "right" => Some(Float::FloatRight),
                _ => panic!("Wrong float value"),
            },
            _ => None
        }
    }

    pub fn clear_value(&self) -> Option<Clear> {
        match self.value("clear") {
            Some(Value::Keyword(s)) => match s.as_slice() {
                "left" => Some(Clear::ClearLeft),
                "right" => Some(Clear::ClearRight),
                "both" => Some(Clear::ClearBoth),
                _ => panic!("Wrong clear value"),
            },
            _ => None
        }
    }

    pub fn background_color(&self) -> Color {
        assert!(self.tag_name() == "html");
        if let Some(Value::ColorValue(color)) = self.value("background-color") {
            return color;
        }
        Color { r: 255, g: 255, b: 255, a: 255 }
    }

    pub fn tag_name(&self) -> String {
        match self.node.node_type {
            NodeType::Element(ref data) => data.tag_name.clone(),
            NodeType::Text(ref string) => {
                let mut text = "text: ".to_string();
                if string.len() > 3 {
                    text.push_str(string.slice(0, 3));
                } else {
                    text.push_str(string.as_slice());
                }
                text
            }
        }
    }

    pub fn get_string_if_text_node(&self) -> Option<&str> {
        match self.node.node_type {
            NodeType::Text(ref string) => {
                if !NONE_DISPLAY.contains(&self.tag_name().as_slice()) {
                    return Some(string.as_slice());
                };
            },
            _ => return None,
        }
        return None;
    }

    pub fn check_none_diplay_node(&mut self) {
        if NONE_DISPLAY.contains(&self.tag_name().as_slice()) {
            self.specified_values.insert("display".to_string(), Value::Keyword("none".to_string()));
        };
    }
}

/// Apply a stylesheet to an entire DOM tree, returning a StyledNode tree.
///
/// This finds only the specified values at the moment. Eventually it should be extended to find the
/// computed values too, including inherited values.
pub fn style_tree<'a>(root: &'a Rc<Node>, stylesheet: &'a Stylesheet, inherits: &PropertyMap) -> StyledNode<'a> {
    let values = match root.node_type {
        NodeType::Element(ref elem) => specified_values(root.clone(), elem, stylesheet, inherits),
        NodeType::Text(_) => HashMap::new()
    };
    let new_inherits = get_inherit_style(&values);

    let mut new_style_node = StyledNode {
        node: root.clone(),
        specified_values: values,
        children: root.children.iter().map(|child| style_tree(child, stylesheet, &new_inherits)).collect(),
    };

    new_style_node.check_none_diplay_node();
    new_style_node
}

/// Apply styles to a single element, returning the specified styles.
///
/// To do: Allow multiple UA/author/user stylesheets, and implement the cascade.
fn specified_values(node: Rc<Node>, elem: &ElementData, stylesheet: &Stylesheet, inherits: &PropertyMap) -> PropertyMap {
    let mut values = HashMap::new();
    let mut rules = matching_rules(node, elem, stylesheet);

    // Go through the rules from lowest to highest specificity.
    rules.sort_by(|&(a, _), &(b, _)| a.cmp(&b));
    for &(_, rule) in rules.iter() {
        for declaration in rule.declarations.iter() {
            values.insert(declaration.name.clone(), declaration.value.clone());
        }
    }

    apply_inline_style(&mut values, elem);
    apply_inherit_style(&mut values, inherits);
    return values;
}

fn apply_inherit_style(values: &mut PropertyMap, inherits: &PropertyMap) {
    for (name, value) in inherits.iter() {
        if let None  = values.get(name) {
            values.insert(name.clone(), value.clone());
        };
    }
}

fn get_inherit_style(values: &PropertyMap) -> PropertyMap {
    let mut inherits = HashMap::new();
    for (name, value) in values.iter() {
        if DEFAULT_INHERIT.contains(&name.as_slice()) {
            inherits.insert(name.clone(), value.clone());
        }
    }
    inherits
}

/// A single CSS rule and the specificity of its most specific matching selector.
type MatchedRule<'a> = (Specificity, &'a Rule);

/// Find all CSS rules that match the given element.
fn matching_rules<'a>(node: Rc<Node>, elem: &ElementData, stylesheet: &'a Stylesheet) -> Vec<MatchedRule<'a>> {
    // For now, we just do a linear scan of all the rules.  For large
    // documents, it would be more efficient to store the rules in hash tables
    // based on tag name, id, class, etc.
    stylesheet.rules.iter().filter_map(|rule| match_rule(node.clone(), elem, rule)).collect()
}

/// If `rule` matches `elem`, return a `MatchedRule`. Otherwise return `None`.
fn match_rule<'a>(node: Rc<Node>, elem: &ElementData, rule: &'a Rule) -> Option<MatchedRule<'a>> {
    // Find the first (most specific) matching selector.
    rule.selectors.iter().find(|selector| matches(node.clone(), elem, *selector))
        .map(|selector| (selector.specificity(), rule))
}

/// Selector matching:
fn matches(node: Rc<Node>, elem: &ElementData, selector: &Selector) -> bool {
    match *selector {
        Selector::Simple(ref simple_selector) => matches_simple_selector(elem, simple_selector),
        Selector::Descendant(ref descendant_selector) => matches_descendant_selector(node, elem, descendant_selector.as_slice())
    }
}

fn matches_simple_selector(elem: &ElementData, selector: &SimpleSelector) -> bool {
    // Check type selector
    if selector.tag_name.iter().any(|name| elem.tag_name != *name) {
        return false;
    }

    // Check ID selector
    if selector.id.iter().any(|id| elem.id() != Some(id)) {
        return false;
    }

    // Check class selectors
    let elem_classes = elem.classes();
    if selector.class.iter().any(|class| !elem_classes.contains(&class.as_slice())) {
        return false;
    }

    // We didn't find any non-matching selector components.
    return true;
}

fn matches_descendant_selector(node: Rc<Node>, elem: &ElementData, selector: &[SimpleSelector]) -> bool {
    assert!(selector.len() > 1);

    if !matches_simple_selector(elem, selector.last().unwrap()) {
        return false;
    }

    let current_selector = selector.slice(0, selector.len() - 1);
    return matches_ancestor(node, current_selector);
}

fn matches_ancestor(node: Rc<Node>, selector: &[SimpleSelector]) -> bool {
    let mut current_node = node;
    let mut matching_node: Option<Rc<Node>> = None;
    loop {
        match get_parent(&current_node) {
            Some(parent_node) => {
                if let NodeType::Element(ref parent_elem) = parent_node.node_type {
                    if matches_simple_selector(parent_elem, selector.last().unwrap()) {
                        matching_node = Some(parent_node.clone());
                        break;
                    }
                    current_node = parent_node.clone();
                }
            },
            None => break,
        }
    }

    match matching_node {
        Some(_) => if selector.len() == 1 {
            return true
        },
        None => return false,
    }

    return matches_ancestor(matching_node.unwrap(), selector.slice(0, selector.len() - 1));
}

fn get_parent(node: &Rc<Node>) -> Option<Rc<Node>> {
    if node.parent.borrow().is_empty() {
        return None;
    }
    node.parent.borrow().last().unwrap().upgrade()
}

fn apply_inline_style(values: &mut PropertyMap, elem: &ElementData) {
    if let Some(style_string) = elem.attributes.get("style") {
        let mut last_idx;
        let mut source = style_string.clone();
        if source.char_at(source.len() - 1) != ';' {
            last_idx = source.len();
            source.insert(last_idx, ';');
        }
        // insert {}
        source.insert(0, '{');
        last_idx = source.len();
        source.insert(last_idx, '}');

        for decl in css::parse_inline_style(source).into_iter() {
            values.insert(decl.name, decl.value);
        }
    }
}

pub fn show(style_node: &StyledNode, depth: usize) {
    dom::show(&style_node.node);

    for (key, value) in style_node.specified_values.iter() {
        match *value {
            Value::Keyword(ref value_string) => println!("{}: {}", key, value_string),
            Value::Length(ref len, ref unit) => {
                let unit_string = match unit {
                    &css::Unit::Px => "px",
                    &css::Unit::Em => "em",
                    &css::Unit::Percent => "%",
                    &css::Unit::Default => "",
                };
                println!("{}: {}{}", key, len, unit_string);
            }
            Value::ColorValue(ref col) => println!("{}: {} {} {}", key, col.r, col.g, col.b),
        }
    }

    for i in style_node.children.iter() {
        show(i, depth + 1);
    }
}
