//! Basic DOM data structures.

use std::collections::{HashMap,HashSet};

use std::rc::Rc;
use std::rc::Weak;
use std::cell::RefCell;

pub type AttrMap = HashMap<String, String>;

#[derive(Show)]
pub struct Node {
    pub parent: RefCell<Vec<Weak<Node>>>,

    // data common to all nodes:
    pub children: Vec<Rc<Node>>,

    // data specific to each node type:
    pub node_type: NodeType,
}

#[derive(Show)]
pub enum NodeType {
    Element(ElementData),
    Text(String),
}

#[derive(Show)]
pub struct ElementData {
    pub tag_name: String,
    pub attributes: AttrMap,
}

// Constructor functions for convenience:
pub fn text(data: String) -> Rc<Node> {
    Rc::new(Node { parent: RefCell::new(Vec::new()), children: vec![], node_type: NodeType::Text(data.trim().to_string()) })
}

pub fn elem(name: String, attrs: AttrMap, children: Vec<Rc<Node>>) -> Node {
    Node {
        parent: RefCell::new(Vec::new()),
        children: children,
        node_type: NodeType::Element(ElementData {
            tag_name: name,
            attributes: attrs,
        })
    }
}

// Element methods

impl ElementData {
    pub fn id(&self) -> Option<&String> {
        self.attributes.get("id")
    }

    pub fn classes(&self) -> HashSet<&str> {
        match self.attributes.get("class") {
            Some(classlist) => classlist.as_slice().split(' ').collect(),
            None => HashSet::new()
        }
    }
}

pub fn find_style(node: &Rc<Node>) -> String {
    let mut style_string = String::new();

    if let NodeType::Element(ref data) = node.node_type {
        if data.tag_name == "style" {
            for child in node.children.iter() {
                if let NodeType::Text(ref text) = child.node_type {
                    return text.clone();
                }
            }
        }
    }

    for child in node.children.iter() {
        style_string = find_style(child);
        if !style_string.is_empty() {
            return style_string;
        }
    }

    style_string
}

pub fn show_all(node: &Rc<Node>, depth: usize) {
    for i in range(0us, depth) {
        print!("--");
    }

    show(node);

    for i in node.children.iter() {
        show_all(i, depth + 1);
    }
}

pub fn show(node: &Rc<Node>) {
    match node.node_type {
        NodeType::Element(ref data) => print!(" Element: {}", data.tag_name),
        NodeType::Text(ref string) => print!(" Text: {}", string),
    }

    if node.parent.borrow().is_empty() {
        println!(" -> No parent");
    } else {
        match node.parent.borrow().last().unwrap().upgrade().unwrap().node_type {
            NodeType::Element(ref data) => println!(" -> parent: {}", data.tag_name),
            NodeType::Text(ref string) => println!(" -> parent: {}", string),
        }
    }
}
