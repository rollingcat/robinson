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
    Rc::new(Node { parent: RefCell::new(Vec::new()), children: vec![], node_type: NodeType::Text(data) })
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

pub fn show(node: &Rc<Node>, depth: usize, parent: Option<Weak<Node>>) {
    for i in range(0us, depth) {
        print!("--");
    }

    match node.node_type {
        NodeType::Element(ref data) => println!(" Element: {}", data.tag_name),
        NodeType::Text(ref string) => println!(" Text: {}", string),
    }

    if let Some(unwrap_parent) = parent {
        node.parent.borrow_mut().push(unwrap_parent);
    }

    for i in node.children.iter() {
        show(i, depth + 1, Some(node.clone().downgrade()));
    }
}
