use layout::{AnonymousBlock, BlockNode, InlineNode, FloatNode, TextNode, LayoutBox, Rect};
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

use font::{TextDecoration, FontInfo, Glyph, Text_Dimension, get_glyph, calculate_text_dimension, kerning_offset};

use std::mem;
use std::ptr;
use std::slice;
use std::default::Default;

#[derive(Default, Show)]
pub struct Canvas {
    pub pixels: Vec<Color>,
    pub width: usize,
    pub height: usize,
}

/// Paint a tree of LayoutBoxes to an array of pixels.
pub fn paint(layout_root: &LayoutBox, bounds: Rect, background_color: Color) -> Canvas {
    let display_list = build_display_list(layout_root);
    let mut canvas = Canvas::new(bounds.width as usize, bounds.height as usize, background_color);
    for item in display_list.iter() {
        canvas.paint_item(item);
    }
    return canvas;
}

#[derive(Show)]
enum DisplayCommand {
    SolidColor(Color, Rect),
    Text(String, Rect, FontInfo),
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
    render_text(list, layout_box);

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

fn render_text(list: &mut DisplayList, layout_box: &LayoutBox) {
    if let TextNode(ref text) = layout_box.box_type {
        list.push(DisplayCommand::Text(text.clone(), layout_box.dimensions.content, layout_box.font_info));
    }
}

/// Return the specified color for CSS property `name`, or None if no color was specified.
fn get_color(layout_box: &LayoutBox, name: &str) -> Option<Color> {
    match layout_box.box_type {
        BlockNode(style) | InlineNode(style) | FloatNode(style) => match style.value(name) {
            Some(Value::ColorValue(color)) => Some(color),
            _ => None
        },
        TextNode(_) | AnonymousBlock => None
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
            },
            &DisplayCommand::Text(ref string, ref rect, ref font_info) => {
                self.paint_text(string.as_slice(), rect, font_info);
            }
        }
    }

    fn paint_text(&mut self, string: &str, rect: &Rect, font_info: &FontInfo) {
        let handle = FontContextHandle::new();
        let start_idx = rect.y as usize * self.width + rect.x as usize;

        unsafe {
            let mut face: FT_Face = ptr::null_mut();
            let mut error: FT_Error;
            let filename = "./examples/verdana.ttf".as_ptr() as *mut i8;
            error = FT_New_Face(handle.ctx.ctx, filename, 0, &mut face);

            if error != 0 || face.is_null() {
                println!("failed to new face");
                return;
            }

            error = FT_Set_Pixel_Sizes(face, 0, font_info.size as u32);
            if error != 0 {
                println!("failed to set pixel size: {}", font_info.size);
                return;
            }

            let mut text_dimension = calculate_text_dimension(string.as_slice(), &face);
            text_dimension.height = font_info.size;
            text_dimension.baseline = calculate_text_dimension("g", &face).baseline;

            let mut pen = struct_FT_Vector_ { x: 0, y: 0 };
            let mut c: char;
            let mut pc: char = 0 as char;

            let mut text_canvas = Canvas::new(text_dimension.width as usize, font_info.line_height as usize, Color { r: 0, g: 0, b: 0, a: 0 });

            for c in string.chars() {
                let glyph = get_glyph(c, &face, true);

                pen.x += kerning_offset(c, pc, &face) as i64;

                let bearing = (font_info.line_height - text_dimension.height) / 2;
                pen.y = (font_info.line_height - glyph.ascent - text_dimension.baseline - bearing) as i64;

                text_canvas.paint_char(&glyph, pen.x, pen.y, &text_dimension);

                pen.x += glyph.advance_width as i64;

                pc = c;
            }

            text_canvas.paint_text_decoration(font_info);

            for y in range(0, text_canvas.height) {
                for x in range(0, text_canvas.width) {
                    let src_col = text_canvas.pixels[y * text_canvas.width + x];
                    let dst_col = self.pixels[start_idx + y * self.width + x];

                    let dst: &mut Color = &mut self.pixels[start_idx + y * self.width + x];

                    dst.r = ((dst_col.r as f32 * (255 - src_col.a) as f32 / 255.0) + (font_info.color.r as f32 * src_col.a as f32 / 255.0)) as u8;
                    dst.g = ((dst_col.g as f32 * (255 - src_col.a) as f32 / 255.0) + (font_info.color.g as f32 * src_col.a as f32 / 255.0)) as u8;
                    dst.b = ((dst_col.b as f32 * (255 - src_col.a) as f32 / 255.0) + (font_info.color.b as f32 * src_col.a as f32 / 255.0)) as u8;
                }
            }
        }
    }

    fn paint_char(&mut self, glyph: &Glyph, x: i64, y: i64, text_info: &Text_Dimension) {
        let mut src: usize = 0;
        let mut dst: usize = (y * text_info.width as i64 + x) as usize;
        dst += glyph.bearing_x as usize;

        let row_offset = (text_info.width - (*glyph).width) as usize;

        for sy in range(0, (*glyph).height) {
            for sx in range(0, (*glyph).width) {
                self.pixels[dst] = (*glyph).pixelmap.pixels[src];
                src += 1;
                dst += 1;
            }
            dst += row_offset;
        }
    }

    fn paint_text_decoration(&mut self, font_info: &FontInfo) {
        if font_info.deco != TextDecoration::Underline {
            return;
        }

        let pos = (self.width * (self.height - 2)) as usize;
        for i in range(0, self.width) {
            self.pixels[pos + i] = font_info.color;
        }
    }
}

trait FloatClamp : Float {
    fn clamp(self, lower: Self, upper: Self) -> Self {
        self.max(lower).min(upper)
    }
}
impl<T: Float> FloatClamp for T {}
