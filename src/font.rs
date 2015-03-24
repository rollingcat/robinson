
use std::default::Default;

use font_context::FontContextHandle;
use freetype::freetype::{FT_Face, FT_New_Face, FT_Done_Face, FT_Error};
use freetype::freetype::{FT_Get_Char_Index, FT_Set_Char_Size, FT_Load_Glyph, FT_GlyphSlot};
use freetype::freetype::{FT_UInt, FT_ULong, FT_Vector, struct_FT_Vector_};
use freetype::freetype::{FT_Load_Char, FT_LOAD_RENDER};
use freetype::freetype::{FT_Get_Kerning, FT_KERNING_DEFAULT};
use freetype::freetype::{FT_Bitmap};

use painting::{Canvas};
use color::{Color};

use std::mem;
use std::ptr;
use std::slice;

#[derive(Show, Clone, PartialEq)]
pub enum TextDecoration {
    Normal,
    Underline,
    Overline,
    LineThrough,
}

impl Default for TextDecoration {
    fn default() -> TextDecoration {
        TextDecoration::Normal
    }
}

impl Copy for TextDecoration {}

#[derive(Show, Clone, Default)]
pub struct FontInfo {
    pub size: i32,
    pub line_height: i32,
    pub color: Color,
    pub deco: TextDecoration,
}

impl Copy for FontInfo {}

#[derive(Default, Show)]
pub struct Glyph {
    pub top: i32,
    pub height: i32,
    pub width: i32,
    pub descent: i32,
    pub ascent: i32,
    pub advance_width: i32,
    pub bearing_x: i32,
    pub pixelmap: Canvas
}

#[derive(Default, Show)]
pub struct Text_Dimension {
    pub width: i32,
    pub height: i32,
    pub baseline: i32,
    // unsigned char *outpuffer;
}

pub fn get_glyph(character: char, face: &FT_Face, bBitmap: bool) -> Glyph {
    unsafe {
        let error = FT_Load_Char(*face, character as u64, FT_LOAD_RENDER);
        if error != 0 {
            panic!("failed to load char: {}", character);
        }

        let slot: FT_GlyphSlot = mem::transmute((**face).glyph);
        return convert_glyph(&slot, bBitmap);
    }
}

pub fn calculate_text_dimension(text: &str, face: &FT_Face) -> Text_Dimension {
    let mut width;
    let mut max_ascent;
    let mut max_descent;
    let mut kerning_x;

    let mut character: char;
    let mut prev_character: char;

    let mut result: Text_Dimension = Default::default();
    let kerning: FT_Vector;

    width = 0;
    max_ascent = 0;
    max_descent = 0;
    prev_character = 0 as char;

    for character in text.chars() {
        let glyph = get_glyph(character, face, false);
        if (max_ascent < glyph.ascent) {
            max_ascent = glyph.ascent;
        }
        if (max_descent < glyph.descent) {
            max_descent = glyph.descent;
        }

        kerning_x = kerning_offset(character, prev_character, face);

        if ((glyph.advance_width + kerning_x) < (glyph.width + kerning_x)) {
            width += (glyph.width + kerning_x);
        } else {
            width += (glyph.advance_width + kerning_x);
        }

        prev_character = character;
    }
    result.height = max_ascent + max_descent;
    result.width  = width;
    result.baseline = max_descent;

    return result;
}

pub fn kerning_offset(c: char, pc: char, face: &FT_Face) -> i32 {
    let mut kerning = struct_FT_Vector_ { x: 0, y: 0 };

    unsafe {
        let error = FT_Get_Kerning(*face, c as u32, pc as u32, FT_KERNING_DEFAULT, &mut kerning);

        if error != 0 {
            println!("failed to get kerning");
        }
    }

    return kerning.x as i32 / 64;
}

fn convert_glyph(slot: &FT_GlyphSlot, bBitmap: bool) -> Glyph {
    let mut glyph_data: Glyph = Default::default();
    unsafe {
        if bBitmap {
            glyph_data.pixelmap = draw_char(&(**slot).bitmap);
        }
        glyph_data.width = (**slot).bitmap.width;
        glyph_data.height = (**slot).bitmap.rows;
        glyph_data.top = (**slot).bitmap_top;
        glyph_data.advance_width = (**slot).advance.x as i32 / 64;
        glyph_data.bearing_x = (**slot).metrics.horiBearingX as i32 / 64;
    }

    let mut descent = 0;
    let mut ascent = 0;
    let mut ascent_calc = 0;

    if (descent < (glyph_data.height - glyph_data.top)) {
        descent = glyph_data.height - glyph_data.top;
    }
    glyph_data.descent = descent;

    if (glyph_data.top < glyph_data.height) {
        ascent_calc = glyph_data.height;
    } else {
        ascent_calc = glyph_data.top;
    }
    if (ascent < (ascent_calc - descent)) {
        ascent = ascent_calc - descent;
    }
    glyph_data.ascent = ascent;

    glyph_data
}

fn draw_char(bitmap: &FT_Bitmap) -> Canvas {
    let mut canvas = Canvas::new(bitmap.width as usize, bitmap.rows as usize, Color { r: 0, g: 0, b: 0, a: 0 });

    unsafe {
        let s: &mut [u8] = slice::from_raw_mut_buf(&bitmap.buffer, (bitmap.width * bitmap.rows) as usize);

        for x in range(0, bitmap.width) {
            for y in range(0, bitmap.rows) {
                let idx = (y * bitmap.width + x) as usize;
                let value = s[idx];

                canvas.pixels[idx].a = value;
            }
        }
    }

    return canvas;
}
