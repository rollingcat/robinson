///! Basic CSS block layout.

use style::{StyledNode, Display, Float, Clear};
use css::{Value};
use css::Value::{Keyword, Length};
use css::Unit::Px;
use std::default::Default;
use std::iter::AdditiveIterator; // for `sum`

use dom::{NodeType};

pub use self::BoxType::{AnonymousBlock, InlineNode, BlockNode, FloatNode, TextNode};

use font_context::FontContextHandle;
use freetype::freetype::{FT_Face, FT_New_Face, FT_Done_Face, FT_Error};
use freetype::freetype::{FT_Get_Char_Index, FT_Set_Pixel_Sizes, FT_Load_Glyph, FT_GlyphSlot};
use freetype::freetype::{FT_UInt, FT_ULong, FT_Vector, struct_FT_Vector_};
use freetype::freetype::{FT_Load_Char, FT_LOAD_RENDER};
use freetype::freetype::{FT_Get_Kerning, FT_KERNING_DEFAULT};

use font::{FontInfo, Glyph, Text_Dimension, get_glyph, calculate_text_dimension};

use std::ptr;
use std::mem;

// CSS box model. All sizes are in px.

#[derive(Default, Show, Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Default)]
pub struct Dimensions {
    /// Position of the content area relative to the document origin:
    pub content: Rect,
    // Surrounding edges:
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
}

#[derive(Default, Show)]
pub struct EdgeSizes {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Copy for Rect {}
impl Copy for Dimensions {}
impl Copy for EdgeSizes {}

/// A node in the layout tree.
pub struct LayoutBox<'a> {
    pub dimensions: Dimensions,
    pub box_type: BoxType<'a>,
    pub children: Vec<LayoutBox<'a>>,
    pub float_info: FloatInfo,
    pub font_info: FontInfo,
}

pub enum BoxType<'a> {
    BlockNode(&'a StyledNode<'a>),
    InlineNode(&'a StyledNode<'a>),
    FloatNode(&'a StyledNode<'a>),
    TextNode(String),
    AnonymousBlock,
}

#[derive(Default, Show)]
pub struct FloatInfo {
    pub left_float_max_y: f32,
    pub right_float_max_y: f32,
}

impl<'a> LayoutBox<'a> {
    fn new(box_type: BoxType) -> LayoutBox {
        LayoutBox {
            box_type: box_type,
            dimensions: Default::default(),
            children: Vec::new(),
            float_info: Default::default(),
            font_info: Default::default(),
        }
    }

    pub fn get_style_node(&self) -> &'a StyledNode<'a> {
        match self.box_type {
            BlockNode(node) => node,
            InlineNode(node) => node,
            FloatNode(node) => node,
            TextNode(_) => panic!("text node box has no style node"),
            AnonymousBlock => panic!("Anonymous block box has no style node")
        }
    }
}

/// Transform a style tree into a layout tree.
pub fn layout_tree<'a>(node: &'a StyledNode<'a>, mut containing_block: Dimensions) -> LayoutBox<'a> {
    // The layout algorithm expects the container height to start at 0.
    // TODO: Save the initial containing block height, for calculating percent heights.
    containing_block.content.height = 0.0;

    let mut root_box = build_layout_tree(node);

    let mut float_list: Vec<(Float, Dimensions)> = Vec::new();
    let mut previous_inline: Option<(i32, i32)> = None;
    root_box.layout(containing_block, &mut float_list, &mut previous_inline);
    return root_box;
}

/// Build the tree of LayoutBoxes, but don't perform any layout calculations yet.
fn build_layout_tree<'a>(style_node: &'a StyledNode<'a>) -> LayoutBox<'a> {
    // Create the root box.
    let mut root = create_layout_box(style_node);

    // Create the descendant boxes.
    for child in style_node.children.iter() {
        match child.display() {
            Display::Block => root.children.push(build_layout_tree(child)),
            Display::Inline => root.get_inline_container().children.push(build_layout_tree(child)),
            Display::None => {} // Don't lay out nodes with `display: none;`
        }
    }
    return root;
}

fn create_layout_box<'a>(style_node: &'a StyledNode<'a>) -> LayoutBox<'a> {
    if let Some(_) = style_node.value("float") {
        return LayoutBox::new(FloatNode(style_node));
    }

    LayoutBox::new(match style_node.display() {
        Display::Block => BlockNode(style_node),
        Display::Inline => InlineNode(style_node),
        Display::None => panic!("Root node has display: none.")
    })
}

impl<'a> LayoutBox<'a> {
    /// Lay out a box and its descendants.
    fn layout(&mut self, containing_block: Dimensions, float_list: &mut Vec<(Float, Dimensions)>, previous_inline: &mut Option<(i32, i32)>) {
        match self.box_type {
            BlockNode(_) => self.layout_block(containing_block, float_list, previous_inline),
            InlineNode(_) => self.layout_inline(containing_block, float_list, previous_inline),
            FloatNode(_) => self.layout_float(containing_block, &mut Default::default(), None, float_list, previous_inline),
            TextNode(_) => self.layout_text(containing_block, Default::default(), previous_inline),
            AnonymousBlock => self.layout_anonymous(containing_block, Default::default(), float_list, previous_inline),
        }
    }

    fn fill_font_info(&mut self) {
        match self.box_type {
            BlockNode(style) | InlineNode(style) | FloatNode(style) => {
                if let Some(Value::ColorValue(color)) = style.value("color") {
                    self.font_info.color = color;
                }
                if let Some(val) = style.value("font-size") {
                    self.font_info.size = val.to_px().unwrap() as i32;
                }
                if let Some(val) = style.value("line-height") {
                    self.font_info.line_height = val.to_px().unwrap() as i32;
                }
            },
            TextNode(_) | AnonymousBlock => {
                panic!("wrong function call!");
            }
        }
    }

    fn copy_font_info(&mut self, font_info: &FontInfo) {
        match self.box_type {
            BlockNode(_) | InlineNode(_) | FloatNode(_) => {
                panic!("wrong function call!");
            },
            TextNode(_) | AnonymousBlock => {
                self.font_info = *font_info;
            }
        }
    }

    /// Lay out a block-level element and its descendants.
    fn layout_block(&mut self, containing_block: Dimensions, float_list: &mut Vec<(Float, Dimensions)>, previous_inline: &mut Option<(i32, i32)>) {
        self.fill_font_info();
        // Child width can depend on parent width, so we need to calculate this box's width before
        // laying out its children.
        self.calculate_block_width(containing_block);

        // Determine where the box is located within its container.
        self.calculate_block_position(containing_block);

        // Recursively lay out the children of this box.
        self.layout_block_children(float_list, previous_inline);

        // Parent height can depend on child height, so `calculate_height` must be called after the
        // children are laid out.
        self.calculate_block_height();
    }

    fn layout_float(&mut self, containing_block: Dimensions,
                    float_rect: &mut Rect,
                    previous_float: Option<Dimensions>,
                    float_list: &mut Vec<(Float, Dimensions)>,
                    previous_inline: &mut Option<(i32, i32)>) {
        self.fill_font_info();

        self.calculate_float_width(containing_block);

        self.calculate_float_position(containing_block, float_rect);

        let mut shift = previous_float;
        loop {
            self.shift_float_by_container_width(containing_block, float_rect, shift);
            shift = self.shift_float_by_other_floats(float_rect, &previous_float, float_list);
            if let None = shift {
                float_rect.x += self.dimensions.margin_box().width;
                break;
            }
        }

        self.layout_block_children(float_list, previous_inline);

        self.calculate_float_height();

        float_list.push((self.get_style_node().float_value().unwrap(), self.dimensions));
    }

    fn layout_inline(&mut self, containing_block: Dimensions, float_list: &mut Vec<(Float, Dimensions)>, previous_inline: &mut Option<(i32, i32)>) {
        self.fill_font_info();
        // Child width can depend on parent width, so we need to calculate this box's width before
        // laying out its children.
        self.calculate_inline_width(containing_block, previous_inline);

        // Determine where the box is located within its container.
        self.calculate_inline_position(containing_block, previous_inline);

        // Recursively lay out the children of this box.
        self.layout_block_children(float_list, previous_inline);

        // Parent height can depend on child height, so `calculate_height` must be called after the
        // children are laid out.
        self.calculate_block_height();
    }

    fn layout_text(&mut self, containing_block: Dimensions, font_info: FontInfo, previous_inline: &mut Option<(i32, i32)>) {
        self.copy_font_info(&font_info);

        let mut text = String::new();
        if let TextNode(ref s) = self.box_type {
            text.push_str(s.as_slice());
        } else {
            panic!("Self is not a TextNode");
        }

        let d = &mut self.dimensions;

        unsafe {
            let handle = FontContextHandle::new();
            let mut face: FT_Face = ptr::null_mut();
            let mut error: FT_Error;
            let filename = "/usr/share/fonts/truetype/msttcorefonts/verdana.ttf".as_ptr() as *mut i8;
            error = FT_New_Face(handle.ctx.ctx, filename, 0, &mut face);

            if error != 0 || face.is_null() {
                println!("failed to new face");
            }

            error = FT_Set_Pixel_Sizes(face, 0, 10);
            if error != 0 {
                println!("failed to set pixel size");
            }

            let text_dimension = calculate_text_dimension(text.as_slice(), &face);

            d.content.width = text_dimension.width as f32;
            d.content.height = font_info.line_height as f32;

            if let Some((inline_x, inline_y)) = *previous_inline {
                d.content.x = inline_x as f32;
                d.content.y = inline_y as f32;
                if d.content.max_x() > containing_block.content.max_x() {
                    d.content.x = containing_block.content.x;
                    d.content.y += d.content.height;
                }
            } else {
                d.content.x = containing_block.content.x;
                d.content.y = containing_block.content.y;
            }

        }
    }

    fn layout_anonymous(&mut self, containing_block: Dimensions, font_info: FontInfo, float_list: &mut Vec<(Float, Dimensions)>, previous_inline: &mut Option<(i32, i32)>) {
        self.copy_font_info(&font_info);
        {
            let d = &mut self.dimensions;
            d.content.width = containing_block.content.width;
            d.content.x = containing_block.content.x;
            d.content.y = containing_block.content.y;
        }
        self.layout_block_children(float_list, previous_inline);
    }


    /// Calculate the width of a block-level non-replaced element in normal flow.
    ///
    /// http://www.w3.org/TR/CSS2/visudet.html#blockwidth
    ///
    /// Sets the horizontal margin/padding/border dimensions, and the `width`.
    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let style = self.get_style_node();

        // `width` has initial value `auto`.
        let auto = Keyword("auto".to_string());
        let mut width = style.value("width").unwrap_or(auto.clone());

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        let mut margin_left = style.lookup("margin-left", "margin", &zero);
        let mut margin_right = style.lookup("margin-right", "margin", &zero);

        let border_left = style.lookup("border-left-width", "border-width", &zero);
        let border_right = style.lookup("border-right-width", "border-width", &zero);

        let padding_left = style.lookup("padding-left", "padding", &zero);
        let padding_right = style.lookup("padding-right", "padding", &zero);

        let total = [&margin_left, &margin_right, &border_left, &border_right,
                     &padding_left, &padding_right, &width].iter().map(|v| v.to_px().unwrap_or(v.percent_to_px(containing_block.content.width))).sum();

        // If width is not auto and the total is wider than the container, treat auto margins as 0.
        if width != auto && total > containing_block.content.width {
            if margin_left == auto {
                margin_left = Length(0.0, Px);
            }
            if margin_right == auto {
                margin_right = Length(0.0, Px);
            }
        }

        // Adjust used values so that the above sum equals `containing_block.width`.
        // Each arm of the `match` should increase the total width by exactly `underflow`,
        // and afterward all values should be absolute lengths in px.
        let underflow = containing_block.content.width - total;

        match (width == auto, margin_left == auto, margin_right == auto) {
            // If the values are overconstrained, calculate margin_right.
            (false, false, false) => {
                margin_right = Length(margin_right.to_px().unwrap() + underflow, Px);
            }

            // If exactly one size is auto, its used value follows from the equality.
            (false, false, true) => { margin_right = Length(underflow, Px); }
            (false, true, false) => { margin_left  = Length(underflow, Px); }

            // If width is set to auto, any other auto values become 0.
            (true, _, _) => {
                if margin_left == auto { margin_left = Length(0.0, Px); }
                if margin_right == auto { margin_right = Length(0.0, Px); }

                if underflow >= 0.0 {
                    // Expand width to fill the underflow.
                    width = Length(underflow, Px);
                } else {
                    // Width can't be negative. Adjust the right margin instead.
                    width = Length(0.0, Px);
                    margin_right = Length(margin_right.to_px().unwrap() + underflow, Px);
                }
            }

            // If margin-left and margin-right are both auto, their used values are equal.
            (false, true, true) => {
                margin_left = Length(underflow / 2.0, Px);
                margin_right = Length(underflow / 2.0, Px);
            }
        }

        let d = &mut self.dimensions;
        d.content.width = width.to_px().unwrap_or(width.percent_to_px(containing_block.content.width));

        d.padding.left = padding_left.to_px().unwrap();
        d.padding.right = padding_right.to_px().unwrap();

        d.border.left = border_left.to_px().unwrap();
        d.border.right = border_right.to_px().unwrap();

        d.margin.left = margin_left.to_px().unwrap();
        d.margin.right = margin_right.to_px().unwrap();
    }

    fn calculate_float_width(&mut self, containing_block: Dimensions) {
        let style = self.get_style_node();

        // `width` has initial value `auto`.
        let auto = Keyword("auto".to_string());

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        let d = &mut self.dimensions;
        let mut width = style.value("width").unwrap_or(auto.clone());

        d.padding.left = style.lookup("padding-left", "padding", &zero).to_px().unwrap();
        d.padding.right = style.lookup("padding-right", "padding", &zero).to_px().unwrap();

        d.border.left = style.lookup("border-left-width", "border-width", &zero).to_px().unwrap();
        d.border.right = style.lookup("border-right-width", "border-width", &zero).to_px().unwrap();

        d.margin.left = style.lookup("margin-left", "margin", &zero).to_px().unwrap();
        d.margin.right = style.lookup("margin-right", "margin", &zero).to_px().unwrap();

        if width == auto {
            width = Length(containing_block.content.width - d.padding.left - d.padding.right - d.border.left - d.border.right - d.margin.left - d.margin.right, Px);
        }
        d.content.width = width.to_px().unwrap_or(width.percent_to_px(containing_block.content.width));
    }

    fn calculate_inline_width(&mut self, containing_block: Dimensions, previous_inline: &mut Option<(i32, i32)>) {
        let style = self.get_style_node();

        // `width` has initial value `auto`.
        let auto = Keyword("auto".to_string());

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        {
            let d = &mut self.dimensions;
            let mut width = style.value("width").unwrap_or(auto.clone());

            d.padding.left = style.lookup("padding-left", "padding", &zero).to_px().unwrap();
            d.padding.right = style.lookup("padding-right", "padding", &zero).to_px().unwrap();

            d.border.left = style.lookup("border-left-width", "border-width", &zero).to_px().unwrap();
            d.border.right = style.lookup("border-right-width", "border-width", &zero).to_px().unwrap();

            d.margin.left = style.lookup("margin-left", "margin", &zero).to_px().unwrap();
            d.margin.right = style.lookup("margin-right", "margin", &zero).to_px().unwrap();

            if width == auto {
                let mut width_px = containing_block.content.width - d.padding.left - d.padding.right - d.border.left - d.border.right - d.margin.left - d.margin.right;
                if let Some((inline_x, inline_y)) = *previous_inline {
                    width_px -= inline_x as f32 - containing_block.content.x;
                    println!("calculate_inline_width() - previous_inline: ({} {})", inline_x, inline_y);
                    println!("---> containing_block.content: {:?}", containing_block.content);
                    println!("---> width_px: {:?}", width_px);
                }
                width = Length(width_px, Px);
            }
            d.content.width = width.to_px().unwrap_or(width.percent_to_px(containing_block.content.width));
        }
    }

    /// Finish calculating the block's edge sizes, and position it within its containing block.
    ///
    /// http://www.w3.org/TR/CSS2/visudet.html#normal-block
    ///
    /// Sets the vertical margin/padding/border dimensions, and the `x`, `y` values.
    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let style = self.get_style_node();
        let d = &mut self.dimensions;

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        // If margin-top or margin-bottom is `auto`, the used value is zero.
        d.margin.top = style.lookup("margin-top", "margin", &zero).to_px().unwrap();
        d.margin.bottom = style.lookup("margin-bottom", "margin", &zero).to_px().unwrap();

        d.border.top = style.lookup("border-top-width", "border-width", &zero).to_px().unwrap();
        d.border.bottom = style.lookup("border-bottom-width", "border-width", &zero).to_px().unwrap();

        d.padding.top = style.lookup("padding-top", "padding", &zero).to_px().unwrap();
        d.padding.bottom = style.lookup("padding-bottom", "padding", &zero).to_px().unwrap();

        // Position the box below all the previous boxes in the container.
        d.content.x = containing_block.content.x +
                      d.margin.left + d.border.left + d.padding.left;
        d.content.y = containing_block.content.y + containing_block.content.height +
                      d.margin.top + d.border.top + d.padding.top;
    }

    fn calculate_float_position(&mut self, containing_block: Dimensions, float_rect : &Rect) {
        let style = self.get_style_node();
        let d = &mut self.dimensions;

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        // If margin-top or margin-bottom is `auto`, the used value is zero.
        d.margin.top = style.lookup("margin-top", "margin", &zero).to_px().unwrap();
        d.margin.bottom = style.lookup("margin-bottom", "margin", &zero).to_px().unwrap();

        d.border.top = style.lookup("border-top-width", "border-width", &zero).to_px().unwrap();
        d.border.bottom = style.lookup("border-bottom-width", "border-width", &zero).to_px().unwrap();

        d.padding.top = style.lookup("padding-top", "padding", &zero).to_px().unwrap();
        d.padding.bottom = style.lookup("padding-bottom", "padding", &zero).to_px().unwrap();

        let float_direction = style.float_value();
        assert!(float_direction != None);

        match float_direction.unwrap() {
            Float::FloatLeft => {
                d.content.x =
                containing_block.content.x + d.margin.left + d.border.left + d.padding.left + float_rect.x;
            },
            Float::FloatRight => {
                let self_width_right = d.content.width + d.margin.right + d.border.right + d.padding.right;
                d.content.x =
                containing_block.content.x + containing_block.content.width - self_width_right - float_rect.x;
            },
        }

        d.content.y = containing_block.content.y + containing_block.content.height +
                      d.margin.top + d.border.top + d.padding.top + float_rect.y;
    }

    fn calculate_inline_position(&mut self, containing_block: Dimensions, previous_inline: &mut Option<(i32, i32)>) {
        let style = self.get_style_node();
        let d = &mut self.dimensions;

        // margin, border, and padding have initial value 0.
        let zero = Length(0.0, Px);

        // If margin-top or margin-bottom is `auto`, the used value is zero.
        d.margin.top = style.lookup("margin-top", "margin", &zero).to_px().unwrap();
        d.margin.bottom = style.lookup("margin-bottom", "margin", &zero).to_px().unwrap();

        d.border.top = style.lookup("border-top-width", "border-width", &zero).to_px().unwrap();
        d.border.bottom = style.lookup("border-bottom-width", "border-width", &zero).to_px().unwrap();

        d.padding.top = style.lookup("padding-top", "padding", &zero).to_px().unwrap();
        d.padding.bottom = style.lookup("padding-bottom", "padding", &zero).to_px().unwrap();

        // Position the box below all the previous boxes in the container.
        d.content.x = containing_block.content.x +
                      d.margin.left + d.border.left + d.padding.left;
        d.content.y = containing_block.content.y + containing_block.content.height +
                      d.margin.top + d.border.top + d.padding.top;

        if let Some((inline_x, inline_y)) = *previous_inline {
            println!("calculate_inline_position() - previous_inline: ({} {})", inline_x, inline_y);
            d.content.x = inline_x as f32;
            d.content.y = inline_y as f32;
            println!("---> d.content: {:?}", d.content);
        }
    }

    fn shift_float_by_container_width(&mut self, container: Dimensions, float_rect: &mut Rect, previous_float: Option<Dimensions>) {
        let float_direction = self.get_style_node().float_value();
        let d = &mut self.dimensions;

        if let Some(prev) = previous_float {
            let mut downwards = false;
            match float_direction.unwrap() {
                Float::FloatLeft => {
                    if d.margin_box().max_x() > container.content.max_x() {
                        d.content.x = d.content.x - float_rect.x;
                        downwards = true;
                    }
                },
                Float::FloatRight => {
                    if d.margin_box().x < container.content.x {
                        d.content.x = d.content.x + float_rect.x;
                        downwards = true;
                    }
                },
            };
            if downwards {
                float_rect.x = 0f32;
                let mut diff = prev.margin_box().max_y() - d.margin_box().y;
                d.content.y += diff;
                float_rect.y += diff;
            }
        }
    }

    fn shift_float_by_other_floats(&mut self, float_rect: &mut Rect, previous_float: &Option<Dimensions>, float_list: &Vec<(Float, Dimensions)>) -> Option<Dimensions> {
        let mut shift_by = None;
        let float_direction = self.get_style_node().float_value().unwrap();

        for &(ref other_direction, ref other) in float_list.iter() {
            let mut same_direction = true;
            if self.dimensions.margin_box().intersect(&other.margin_box()) {
                // When intersects with other float.
                match (&float_direction, other_direction) {
                    (&Float::FloatLeft, &Float::FloatLeft) => {
                        let mut diff = self.dimensions.content.x;
                        self.dimensions.content.x = other.margin_box().max_x() + self.dimensions.margin.left + self.dimensions.border.left + self.dimensions.padding.left;
                        diff = self.dimensions.content.x - diff;
                        float_rect.x += diff;
                    },
                    (&Float::FloatRight, &Float::FloatRight) => {
                        let mut diff = self.dimensions.content.x;
                        self.dimensions.content.x = other.margin_box().x - self.dimensions.margin.right - self.dimensions.border.right
                                                    - self.dimensions.padding.right - self.dimensions.content.width;
                        diff = diff - self.dimensions.content.x;
                        float_rect.x += diff;
                    },
                    (_, _) => {
                        let mut diff = self.dimensions.content.y;
                        if let None = *previous_float {
                            self.dimensions.content.y = other.margin_box().max_y() + self.dimensions.margin.top + self.dimensions.border.top + self.dimensions.padding.top;
                        } else {
                            self.dimensions.content.y = previous_float.unwrap().margin_box().max_y() + self.dimensions.margin.top + self.dimensions.border.top + self.dimensions.padding.top;
                        }
                        diff = self.dimensions.content.y - diff;
                        float_rect.y += diff;

                        match float_direction {
                            Float::FloatLeft => self.dimensions.content.x -= float_rect.x,
                            Float::FloatRight => self.dimensions.content.x += float_rect.x,
                        };
                        float_rect.x = 0f32;
                    },
                }
                shift_by = Some(*other);
            }
        }
        return shift_by;
    }

    fn split_text(&mut self, containing_block: Dimensions, font_info: &FontInfo, text: &str, previous_inline: &mut Option<(i32, i32)>) {
        let mut width_px = containing_block.content.width;

        if let Some((inline_x, inline_y)) = *previous_inline {
            width_px -= inline_x as f32 - containing_block.content.x;
            // println!("split_text: {} / previous_inline: ({} {})", text, inline_x, inline_y);
        } else {
            // println!("split_text: {} / previous_inline: None", text);
        }

        // println!("---> width_px: {}", width_px);

        let mut result: Vec<String> = Vec::new();
        let words: Vec<&str> = text.trim().split(' ').collect();

        unsafe {
            let handle = FontContextHandle::new();
            let mut face: FT_Face = ptr::null_mut();
            let mut error: FT_Error;
            let filename = "/usr/share/fonts/truetype/msttcorefonts/verdana.ttf".as_ptr() as *mut i8;
            error = FT_New_Face(handle.ctx.ctx, filename, 0, &mut face);

            if error != 0 || face.is_null() {
                println!("failed to new face");
            }

            error = FT_Set_Pixel_Sizes(face, 0, font_info.size as u32);
            if error != 0 {
                println!("failed to set pixel size");
            }

            let space_width = calculate_text_dimension(" ", &face).width;

            let mut text_width = 0;
            let mut text_chunk = String::new();

            for word in words.iter() {
                let word_dimension = calculate_text_dimension(*word, &face);

                if (text_width + word_dimension.width) >= width_px as i32 {
                    result.push(text_chunk.to_string());
                    text_chunk.clear();
                    text_width = 0;
                    width_px = containing_block.content.width;
                }
                text_width += (word_dimension.width + space_width);
                text_chunk.push_str(*word);
                text_chunk.push(' ');
            }
            if text_chunk.is_empty() == false {
                result.push(text_chunk.to_string());
            }
        }

        for new_str in result.into_iter() {
            self.children.push(LayoutBox::new(TextNode(new_str)));
        }
    }

    /// Lay out the block's children within its content area.
    ///
    /// Sets `self.dimensions.height` to the total content height.
    fn layout_block_children(&mut self, float_list: &mut Vec<(Float, Dimensions)>, previous_inline: &mut Option<(i32, i32)>) {
        let d = &mut self.dimensions;

        let mut left_float_rect: Rect = Default::default();
        let mut right_float_rect: Rect = Default::default();

        let mut previous_left_float: Option<Dimensions> = None;
        let mut previous_right_float: Option<Dimensions> = None;

        let mut b_log = false;
        for child in self.children.iter_mut() {
            // Check clear
            d.content.height += child.calculate_clear_height(&self.float_info, d.content.max_y());

            b_log = false;
            if let AnonymousBlock = self.box_type {
                if let AnonymousBlock = child.box_type {
                    println!("^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^6 b_log TRUE");
                    b_log = true;
                }
            }

            match child.box_type {
                BlockNode(style) => {
                    child.layout_block(*d, float_list, previous_inline);
                    // Increment the height so each child is laid out below the previous one.
                    d.content.height = d.content.height + child.dimensions.margin_box().height;

                    previous_left_float = None;
                    previous_right_float = None;
                    *previous_inline = None;
                },
                FloatNode(style) => {
                    match style.float_value().unwrap() {
                        Float::FloatLeft => {
                            child.layout_float(*d, &mut left_float_rect, previous_left_float, float_list, previous_inline);
                            previous_left_float = Some(child.dimensions);
                            previous_right_float = None;
                        },
                        Float::FloatRight => {
                            child.layout_float(*d, &mut right_float_rect, previous_right_float, float_list, previous_inline);
                            previous_right_float = Some(child.dimensions);
                            previous_left_float = None;
                        },
                    };
                    *previous_inline = None;
                },
                InlineNode(style) => {
                    if let Some(text) = style.get_string_if_text_node() {
                        child.split_text(*d, &self.font_info, text.as_slice(), previous_inline);
                        child.box_type = AnonymousBlock;

                        child.layout_anonymous(*d, self.font_info, float_list, previous_inline);
                    } else {
                        child.layout_inline(*d, float_list, previous_inline);

                        *previous_inline = Some((child.dimensions.margin_box().max_x() as i32, child.dimensions.margin_box().y as i32));
                    }

                    let diff = child.dimensions.margin_box().max_y() - d.content.max_y();
                    if diff > 0f32 { d.content.height += diff; }

                    previous_left_float = None;
                    previous_right_float = None;
                },
                TextNode(_) => {
                    child.layout_text(*d, self.font_info, previous_inline);

                    let diff = child.dimensions.margin_box().max_y() - d.content.max_y();
                    if diff > 0f32 { d.content.height += diff; }

                    *previous_inline = Some((child.dimensions.margin_box().max_x() as i32, child.dimensions.margin_box().y as i32));

                    previous_left_float = None;
                    previous_right_float = None;
                },
                AnonymousBlock => {
                    child.layout_anonymous(*d, self.font_info, float_list, previous_inline);

                    let diff = child.dimensions.margin_box().max_y() - d.content.max_y();
                    if diff > 0f32 { d.content.height += diff; }

                    previous_left_float = None;
                    previous_right_float = None;
                },
            }
            // Update maximum float y
            if child.float_info.left_float_max_y > self.float_info.left_float_max_y {
                self.float_info.left_float_max_y = child.float_info.left_float_max_y;
            }
            if child.float_info.right_float_max_y > self.float_info.right_float_max_y {
                self.float_info.right_float_max_y = child.float_info.right_float_max_y;
            }
        }
    }

    fn calculate_text_size(&mut self, text: &str) -> f32 {
        let d = &mut self.dimensions;
        let handle = FontContextHandle::new();

        let words: Vec<&str> = text.split(' ').collect();

        unsafe {
            let mut face: FT_Face = ptr::null_mut();
            let mut error: FT_Error;
            let filename = "/usr/share/fonts/truetype/msttcorefonts/verdana.ttf".as_ptr() as *mut i8;
            error = FT_New_Face(handle.ctx.ctx, filename, 0, &mut face);

            if error != 0 || face.is_null() {
                println!("failed to new face");
                return 0.0;
            }

            error = FT_Set_Pixel_Sizes(face, 0, self.font_info.size as u32);
            if error != 0 {
                println!("failed to set pixel size");
                return 0.0;
            }

            let space_width = calculate_text_dimension(" ", &face).width;

            let mut text_width = 0;
            let mut text_height = 0;
            let mut max_text_height = 0;
            let mut line_break = false;

            for word in words.iter() {
                let word_dimension = calculate_text_dimension(*word, &face);

                if word_dimension.height > max_text_height {
                    max_text_height = word_dimension.height;
                }

                if (text_width + word_dimension.width) >= d.content.width as i32 {
                    line_break = true;
                    text_height += max_text_height;
                    text_width = word_dimension.width;
                } else {
                    text_width += (word_dimension.width + space_width);
                }
            }

            d.content.height = text_height as f32;
            if line_break == false {
                d.content.width = text_width as f32;
                d.content.height = max_text_height as f32;
            } else {
                d.content.height += max_text_height as f32;
            }
        }

        0.0
    }

    /// Height of a block-level non-replaced element in normal flow with overflow visible.
    fn calculate_block_height(&mut self) {
        let style = self.get_style_node();
        // If the height is set to an explicit length, use that exact length.
        // Otherwise, just keep the value set by `layout_block_children`.
        match style.value("height") {
            Some(value) => { self.dimensions.content.height = value.to_px().unwrap(); }
            _ => {}
        }
    }

    fn calculate_float_height(&mut self) {
        let float_value = self.get_style_node().float_value().unwrap();

        match self.get_style_node().value("height") {
            Some(value) => { self.dimensions.content.height = value.to_px().unwrap(); }
            _ => {
                self.dimensions.content.height +=
                match self.float_info.left_float_max_y > self.float_info.right_float_max_y {
                    true => self.float_info.left_float_max_y - self.dimensions.content.max_y(),
                    false => self.float_info.right_float_max_y - self.dimensions.content.max_y(),
                }
            }
        }

        let height = self.dimensions.margin_box().max_y();
        match float_value {
            Float::FloatLeft => {
                self.float_info.left_float_max_y = height;
                self.float_info.right_float_max_y = 0f32;
            },
            Float::FloatRight => {
                self.float_info.right_float_max_y = height;
                self.float_info.left_float_max_y = 0f32;
            },
        }
    }

    fn calculate_clear_height(&self, float_info: &FloatInfo, current_max_y: f32) -> f32 {
        let mut clear_height = 0f32;

        match self.box_type {
            AnonymousBlock | TextNode(_) => return clear_height,
            _ => {}
        }

        if let Some(clear_value) = self.get_style_node().clear_value() {
            match clear_value {
                Clear::ClearLeft =>
                    if current_max_y < float_info.left_float_max_y {
                        clear_height = float_info.left_float_max_y - current_max_y;
                    },
                Clear::ClearRight =>
                    if current_max_y < float_info.right_float_max_y {
                        clear_height = float_info.right_float_max_y - current_max_y;
                    },
                Clear::ClearBoth => {
                    let float_max_y;
                    if float_info.left_float_max_y > float_info.right_float_max_y {
                        float_max_y = float_info.left_float_max_y;
                    } else {
                        float_max_y = float_info.right_float_max_y;
                    }
                    if current_max_y < float_max_y {
                        clear_height = float_max_y - current_max_y;
                    }
                },
            }
        }
        return clear_height;
    }

    /// Where a new inline child should go.
    fn get_inline_container(&mut self) -> &mut LayoutBox<'a> {
        match self.box_type {
            InlineNode(_) | TextNode(_) | AnonymousBlock => self,
            BlockNode(_) | FloatNode(_) => {
                // If we've just generated an anonymous block box, keep using it.
                // Otherwise, create a new one.
                match self.children.last() {
                    Some(&LayoutBox { box_type: AnonymousBlock,..}) => {}
                    _ => self.children.push(LayoutBox::new(AnonymousBlock))
                }
                self.children.last_mut().unwrap()
            }
        }
    }
}

impl Rect {
    pub fn expanded_by(self, edge: EdgeSizes) -> Rect {
        Rect {
            x: self.x - edge.left,
            y: self.y - edge.top,
            width: self.width + edge.left + edge.right,
            height: self.height + edge.top + edge.bottom,
        }
    }

    pub fn intersect(self, other: &Rect) -> bool {
        return !self.is_empty() && !other.is_empty()
            && self.x < other.max_x() && other.x < self.max_x()
            && self.y < other.max_y() && other.y < self.max_y();
    }

    pub fn is_empty(self) -> bool {
        return self.width == 0f32 && self.height == 0f32;
    }

    pub fn max_x(self) -> f32 {
        return self.x + self.width;
    }

    pub fn max_y(self) -> f32 {
        return self.y + self.height;
    }
}

impl Dimensions {
    /// The area covered by the content area plus its padding.
    pub fn padding_box(self) -> Rect {
        self.content.expanded_by(self.padding)
    }
    /// The area covered by the content area plus padding and borders.
    pub fn border_box(self) -> Rect {
        self.padding_box().expanded_by(self.border)
    }
    /// The area covered by the content area plus padding, borders, and margin.
    pub fn margin_box(self) -> Rect {
        self.border_box().expanded_by(self.margin)
    }
}

pub fn show(node: &LayoutBox, depth: usize) {
    let mut info = String::new();

    for i in range(0us, depth) {
        info.push_str("--");
    }

    let box_type_str = match node.box_type {
        BlockNode(node) => { add_tag_name(&mut info, node); "BlockNode" },
        InlineNode(node) => { add_tag_name(&mut info, node); "InlineNode" },
        FloatNode(node) => { add_tag_name(&mut info, node); "FloatNode" },
        TextNode(ref text) => {
            info.push_str("<text node> ");
            info.push_str(text.as_slice());
            " "
        },
        AnonymousBlock => "AnonymousBlock",
    };
    info.push_str(box_type_str);

    println!("{} : {:?}", info, node.dimensions.content);

    for i in node.children.iter() {
        show(i, depth+1);
    }
}

fn add_tag_name(info: &mut String, node: &StyledNode) {
    info.push('<');
    info.push_str(node.tag_name().as_slice());
    info.push('>'); info.push(' ');
}
