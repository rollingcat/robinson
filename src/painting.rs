use layout::{AnonymousBlock, BlockNode, InlineNode, FloatNode, LayoutBox, Rect};
use css::{Value};
use std::iter::{repeat, range};
use std::num::Float;
use color::{Color};

use font_context::FontContextHandle;
use freetype::freetype::{FT_Face, FT_New_Face, FT_Done_Face};
use freetype::freetype::{FT_Set_Char_Size};
use freetype::freetype::{FT_GlyphSlot};
use freetype::freetype::{FT_Error, FT_Vector, struct_FT_Vector_};
use freetype::freetype::{FT_Set_Transform, FT_Matrix, struct_FT_Matrix_};
use freetype::freetype::{FT_Load_Char, FT_LOAD_RENDER};
use freetype::freetype::{FT_Bitmap, FT_Int, FT_Set_Pixel_Sizes};

use font::{Glyph, Text_Dimension, get_glyph, calculate_text_dimension, kerning_offset};

use std::mem;
use std::ptr;
use std::slice;

#[derive(Default, Show)]
pub struct Canvas {
    pub pixels: Vec<Color>,
    pub width: usize,
    pub height: usize,
}

fn draw_text(glyph: &Glyph, x: i64, y: i64, text_info: &Text_Dimension, canvas: &mut Canvas) {
    // let mut canvas = Canvas::new(text_info.width as usize, text_info.height as usize, Color { r: 255, g: 255, b: 255, a: 255 });

    let mut src: usize = 0;
    let mut dst: usize = (y * text_info.width as i64 + x) as usize;
    let row_offset = (text_info.width - (*glyph).width) as usize;

    for sy in range(0, (*glyph).height) {
        for sx in range(0, (*glyph).width) {
            canvas.pixels[dst] = (*glyph).pixelmap.pixels[src];
            src += 1;
            dst += 1;
        }
        dst += row_offset;
    }

    // canvas
}

/// Paint a tree of LayoutBoxes to an array of pixels.
pub fn paint(layout_root: &LayoutBox, bounds: Rect, background_color: Color) -> Canvas {
    let display_list = build_display_list(layout_root);
    let mut canvas = Canvas::new(bounds.width as usize, bounds.height as usize, background_color);
    for item in display_list.iter() {
        canvas.paint_item(item);
    }

    //----------------------------------------------------------------------------------------
    let handle = FontContextHandle::new();

    unsafe {
        let mut face: FT_Face = ptr::null_mut();
        let mut error: FT_Error;
        let filename = "/usr/share/fonts/truetype/ttf-dejavu/DejaVuSansMono.ttf".as_ptr() as *mut i8;
        error = FT_New_Face(handle.ctx.ctx, filename, 0, &mut face);

        if error != 0 || face.is_null() {
            println!("failed to new face");
            return canvas;
        }

        error = FT_Set_Pixel_Sizes(face, 0, 32);
        if error != 0 {
            println!("failed to set pixel size");
            return canvas;
        }

        let slot: FT_GlyphSlot = mem::transmute((*face).glyph);

        let text = "Hello";
        let text_dimension = calculate_text_dimension(text.as_slice(), &face);

        let mut pen = struct_FT_Vector_ { x: 0, y: 0 };
        let mut c: char;
        let mut pc: char = 0 as char;

        let mut text_canvas = Canvas::new(text_dimension.width as usize, text_dimension.height as usize, Color { r: 0, g: 0, b: 0, a: 255 });

        for c in text.chars() {
            let glyph = get_glyph(c, &face);

            pen.x += kerning_offset(c, pc, &face) as i64;
            pen.y = (text_dimension.height - glyph.ascent - text_dimension.baseline) as i64;

            draw_text(&glyph, pen.x, pen.y, &text_dimension, &mut text_canvas);

            pen.x += glyph.advance_width as i64;

            pc = c;
        }

        canvas.paint_text(&text_canvas);
    }
    //----------------------------------------------------------------------------------------
    return canvas;
}

#[derive(Show)]
enum DisplayCommand {
    SolidColor(Color, Rect),
}

type DisplayList = Vec<DisplayCommand>;

fn build_display_list(layout_root: &LayoutBox) -> DisplayList {
    let mut list = Vec::new();
    render_layout_box(&mut list, layout_root);
    return list;
}

fn render_layout_box(list: &mut DisplayList, layout_box: &LayoutBox) {
    render_background(list, layout_box);
    render_borders(list, layout_box);
    for child in layout_box.children.iter() {
        match child.box_type {
            FloatNode(_) => continue,
            _ => render_layout_box(list, child),
        };
    }

    for child in layout_box.children.iter() {
        if let FloatNode(style_node) = child.box_type {
            render_layout_box(list, child);
        }
    }
}

fn render_background(list: &mut DisplayList, layout_box: &LayoutBox) {
    get_color(layout_box, "background-color").map(|color|
        list.push(DisplayCommand::SolidColor(color, layout_box.dimensions.border_box())));
}

fn render_borders(list: &mut DisplayList, layout_box: &LayoutBox) {
    let color = match get_color(layout_box, "border-color") {
        Some(color) => color,
        _ => return
    };

    let d = &layout_box.dimensions;
    let border_box = d.border_box();

    // Left border
    list.push(DisplayCommand::SolidColor(color, Rect {
        x: border_box.x,
        y: border_box.y,
        width: d.border.left,
        height: border_box.height,
    }));

    // Right border
    list.push(DisplayCommand::SolidColor(color, Rect {
        x: border_box.x + border_box.width - d.border.right,
        y: border_box.y,
        width: d.border.right,
        height: border_box.height,
    }));

    // Top border
    list.push(DisplayCommand::SolidColor(color, Rect {
        x: border_box.x,
        y: border_box.y,
        width: border_box.width,
        height: d.border.top,
    }));

    // Bottom border
    list.push(DisplayCommand::SolidColor(color, Rect {
        x: border_box.x,
        y: border_box.y + border_box.height - d.border.bottom,
        width: border_box.width,
        height: d.border.bottom,
    }));
}

/// Return the specified color for CSS property `name`, or None if no color was specified.
fn get_color(layout_box: &LayoutBox, name: &str) -> Option<Color> {
    match layout_box.box_type {
        BlockNode(style) | InlineNode(style) | FloatNode(style) => match style.value(name) {
            Some(Value::ColorValue(color)) => Some(color),
            _ => None
        },
        AnonymousBlock => None
    }
}

impl Canvas {
    /// Create a blank canvas
    pub fn new(width: usize, height: usize, background_color: Color) -> Canvas {
        return Canvas {
            pixels: repeat(background_color).take(width * height).collect(),
            width: width,
            height: height,
        }
    }

    fn paint_item(&mut self, item: &DisplayCommand) {
        match item {
            &DisplayCommand::SolidColor(color, rect) => {
                // Clip the rectangle to the canvas boundaries.
                let x0 = rect.x.clamp(0.0, self.width as f32) as usize;
                let y0 = rect.y.clamp(0.0, self.height as f32) as usize;
                let x1 = (rect.x + rect.width).clamp(0.0, self.width as f32) as usize;
                let y1 = (rect.y + rect.height).clamp(0.0, self.height as f32) as usize;

                for y in range(y0, y1) {
                    for x in range(x0, x1) {
                        // TODO: alpha compositing with existing pixel
                        self.pixels[y * self.width + x] = color;
                    }
                }
            }
        }
    }

    fn paint_text(&mut self, text_image: &Canvas) {
        for y in range(0, text_image.height) {
            for x in range(0, text_image.width) {
                self.pixels[y * self.width + x] = text_image.pixels[y * text_image.width + x];
            }
        }
    }
}

trait FloatClamp : Float {
    fn clamp(self, lower: Self, upper: Self) -> Self {
        self.max(lower).min(upper)
    }
}
impl<T: Float> FloatClamp for T {}
