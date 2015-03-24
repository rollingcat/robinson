#![allow(unstable)]

extern crate getopts;
extern crate image;
extern crate freetype;
extern crate libc;

use getopts::{optopt,getopts};
use std::default::Default;
use std::io::fs::File;
use std::os::args;
use std::rc::Rc;
use std::collections::HashMap;

mod css;
mod dom;
mod html;
mod layout;
mod style;
mod painting;
mod color;
mod shorthand;
mod font_context;
mod font;

fn main() {
    // Parse command-line options:
    let opts = [
        optopt("h", "html", "HTML document", "FILENAME"),
        optopt("c", "css", "CSS stylesheet", "FILENAME"),
        optopt("o", "output", "Output file", "FILENAME"),
    ];
    let matches = match getopts(args().tail(), &opts) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string())
    };

    // Read input files:
    let read_source = |&: arg_filename: Option<String>, default_filename: &str| {
        let path = match arg_filename {
            Some(ref filename) => &**filename,
            None => default_filename,
        };
        File::open(&Path::new(path)).read_to_string().unwrap()
    };
    let html = read_source(matches.opt_str("h"), "examples/test.html");

    // Since we don't have an actual window, hard-code the "viewport" size.
    let initial_containing_block = layout::Dimensions {
        content: layout::Rect { x: 0.0, y: 0.0, width: 1200.0, height: 800.0 },
        padding: Default::default(),
        border: Default::default(),
        margin: Default::default(),
    };

    // Parsing and rendering:
    let root_node = html::parse(html);
    dom::show_all(&root_node, 1);
    println!("=================================================");
    let css_string = dom::find_style(&root_node);
    let stylesheet = css::parse(css_string);
    // css::show(stylesheet);
    // println!("=================================================");
    let style_root = style::style_tree(&root_node, &stylesheet, &HashMap::new());
    // style::show(&style_root, 1);
    // println!("=================================================");
    let layout_root = layout::layout_tree(&style_root, initial_containing_block);
    layout::show(&layout_root, 1);

    let canvas = painting::paint(&layout_root, initial_containing_block.content, style_root.background_color());

    // Create the output file:
    let filename = matches.opt_str("o").unwrap_or("output.png".to_string());
    let file = File::create(&Path::new(&*filename)).unwrap();

    // Save an image:
    let (w, h) = (canvas.width as u32, canvas.height as u32);
    let buffer: Vec<image::Rgba<u8>> = unsafe { std::mem::transmute(canvas.pixels) };
    let img = image::ImageBuffer::from_fn(w, h, Box::new(|&: x: u32, y: u32| buffer[(y * w + x) as usize]));

    let result = image::ImageRgba8(img).save(file, image::PNG);
    match result {
        Ok(_) => println!("Saved output as {}", filename),
        Err(_) => println!("Error saving output as {}", filename)
    }

    // Debug output:
    // println!("{}", layout_root.dimensions);
    // println!("{}", display_list);
}
