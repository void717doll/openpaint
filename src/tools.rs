use crate::document::{composite_pixel, BlendMode, Document, Pixel};
use std::collections::VecDeque;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    RectangleSelect,
    EllipseSelect,
    Lasso,
    MagicWand,
    MovePixels,
    MoveSelection,
    Pan,
    Zoom,
    Paintbrush,
    Pencil,
    Eraser,
    PaintBucket,
    Gradient,
    ColorPicker,
    CloneStamp,
    Recolor,
    Text,
    Line,
    Rectangle,
    Ellipse,
    RoundedRectangle,
}

impl Tool {
    pub const ALL: [Self; 21] = [
        Self::RectangleSelect,
        Self::MovePixels,
        Self::Lasso,
        Self::MoveSelection,
        Self::EllipseSelect,
        Self::Zoom,
        Self::MagicWand,
        Self::Pan,
        Self::PaintBucket,
        Self::Gradient,
        Self::Paintbrush,
        Self::Eraser,
        Self::Pencil,
        Self::ColorPicker,
        Self::CloneStamp,
        Self::Recolor,
        Self::Text,
        Self::Line,
        Self::Rectangle,
        Self::Ellipse,
        Self::RoundedRectangle,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::RectangleSelect => "Rectangle Select",
            Self::EllipseSelect => "Ellipse Select",
            Self::Lasso => "Lasso Select",
            Self::MagicWand => "Magic Wand",
            Self::MovePixels => "Move Selected Pixels",
            Self::MoveSelection => "Move Selection",
            Self::Pan => "Pan",
            Self::Zoom => "Zoom",
            Self::Paintbrush => "Paintbrush",
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::PaintBucket => "Paint Bucket",
            Self::Gradient => "Gradient",
            Self::ColorPicker => "Color Picker",
            Self::CloneStamp => "Clone Stamp",
            Self::Recolor => "Recolor",
            Self::Text => "Text",
            Self::Line => "Line / Curve",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::RoundedRectangle => "Rounded Rectangle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    Replace,
    Union,
    Subtract,
    Intersect,
    Xor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Reflected,
    Radial,
    Diamond,
    Conical,
}

#[derive(Clone, Copy, Debug)]
pub struct SelectionTransform {
    pub center: (f32, f32),
    pub scale: (f32, f32),
    pub angle: f32,
    pub translation: (f32, f32),
}

impl Default for SelectionTransform {
    fn default() -> Self {
        Self {
            center: (0.0, 0.0),
            scale: (1.0, 1.0),
            angle: 0.0,
            translation: (0.0, 0.0),
        }
    }
}

impl SelectionTransform {
    fn valid(&self) -> bool {
        [
            self.center.0,
            self.center.1,
            self.scale.0,
            self.scale.1,
            self.angle,
            self.translation.0,
            self.translation.1,
        ]
        .iter()
        .all(|value| value.is_finite())
            && self.scale.0 != 0.0
            && self.scale.1 != 0.0
    }

    fn identity(&self) -> bool {
        self.scale == (1.0, 1.0) && self.angle == 0.0 && self.translation == (0.0, 0.0)
    }

    fn rotation(&self) -> (f64, f64) {
        let (mut sine, mut cosine) = (self.angle as f64).sin_cos();
        // exact quarter turns must land on pixel centers without interpolation drift.
        if sine.abs() < 1e-7 {
            sine = 0.0;
        }
        if cosine.abs() < 1e-7 {
            cosine = 0.0;
        }
        if (sine.abs() - 1.0).abs() < 1e-7 {
            sine = sine.signum();
        }
        if (cosine.abs() - 1.0).abs() < 1e-7 {
            cosine = cosine.signum();
        }
        (sine, cosine)
    }

    fn source_point(&self, x: u32, y: u32, rotation: (f64, f64)) -> (f64, f64) {
        let dx = x as f64 + 0.5 - self.center.0 as f64 - self.translation.0 as f64;
        let dy = y as f64 + 0.5 - self.center.1 as f64 - self.translation.1 as f64;
        let (sine, cosine) = rotation;
        (
            (cosine * dx + sine * dy) / self.scale.0 as f64 + self.center.0 as f64,
            (-sine * dx + cosine * dy) / self.scale.1 as f64 + self.center.1 as f64,
        )
    }
}

#[derive(Clone, Debug)]
pub struct ToolSettings {
    pub size: f32,
    pub hardness: f32,
    pub tolerance: u8,
    pub opacity: u8,
    pub filled: bool,
    pub outlined: bool,
    pub gradient: GradientKind,
    pub selection_mode: SelectionMode,
    pub contiguous: bool,
}
impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            size: 8.0,
            hardness: 1.0,
            tolerance: 40,
            opacity: 255,
            filled: false,
            outlined: true,
            gradient: GradientKind::Linear,
            selection_mode: SelectionMode::Replace,
            contiguous: true,
        }
    }
}

fn pixel_at(pixels: &[u8], index: usize) -> Pixel {
    [
        pixels[index * 4],
        pixels[index * 4 + 1],
        pixels[index * 4 + 2],
        pixels[index * 4 + 3],
    ]
}

fn put_pixel(doc: &mut Document, x: i32, y: i32, color: Pixel, coverage: f32, erase: bool) {
    if x < 0 || y < 0 || x >= doc.width as i32 || y >= doc.height as i32 {
        return;
    }
    let index = y as usize * doc.width as usize + x as usize;
    let selected = doc.selection.as_ref().map_or(255, |mask| mask[index]);
    let opacity = (coverage.clamp(0.0, 1.0) * selected as f32).round() as u8;
    if opacity == 0 {
        return;
    }
    let destination = pixel_at(doc.active_pixels(), index);
    let result = if erase {
        let remaining = 1.0 - opacity as f32 / 255.0;
        let alpha = (destination[3] as f32 * remaining).round() as u8;
        if alpha == 0 {
            [0; 4]
        } else {
            [destination[0], destination[1], destination[2], alpha]
        }
    } else {
        composite_pixel(destination, color, opacity, BlendMode::Normal)
    };
    doc.active_pixels_mut()[index * 4..index * 4 + 4].copy_from_slice(&result);
}

fn distance_to_line(point: (f32, f32), from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        ((point.0 - from.0) * dx + (point.1 - from.1) * dy) / length
    } else {
        0.0
    }
    .clamp(0.0, 1.0);
    (point.0 - from.0 - t * dx).hypot(point.1 - from.1 - t * dy)
}

fn brush_coverage(distance: f32, radius: f32, hardness: f32) -> f32 {
    let edge = (radius + 0.5 - distance).clamp(0.0, 1.0);
    let inner = radius * hardness.clamp(0.0, 1.0);
    if distance <= inner || inner >= radius {
        edge
    } else {
        edge * ((radius - distance + 0.5) / (radius - inner + 0.5)).clamp(0.0, 1.0)
    }
}

fn stroke_bounds(
    doc: &Document,
    from: (f32, f32),
    to: (f32, f32),
    radius: f32,
) -> (i32, i32, i32, i32) {
    let left = (from.0.min(to.0) - radius - 1.0).floor().max(0.0) as i32;
    let top = (from.1.min(to.1) - radius - 1.0).floor().max(0.0) as i32;
    let right = (from.0.max(to.0) + radius + 1.0)
        .ceil()
        .min(doc.width as f32) as i32;
    let bottom = (from.1.max(to.1) + radius + 1.0)
        .ceil()
        .min(doc.height as f32) as i32;
    (left, top, right, bottom)
}

pub fn stroke(
    doc: &mut Document,
    from: (f32, f32),
    to: (f32, f32),
    color: Pixel,
    settings: &ToolSettings,
    erase: bool,
) {
    let radius = settings.size.clamp(1.0, 2048.0) / 2.0;
    let (left, top, right, bottom) = stroke_bounds(doc, from, to, radius);
    for y in top..bottom {
        for x in left..right {
            let distance = distance_to_line((x as f32 + 0.5, y as f32 + 0.5), from, to);
            let coverage = brush_coverage(distance, radius, settings.hardness)
                * settings.opacity as f32
                / 255.0;
            put_pixel(doc, x, y, color, coverage, erase);
        }
    }
}

fn color_distance(left: Pixel, right: Pixel) -> u8 {
    // transparent RGB must not prevent a flood through otherwise identical transparent pixels.
    let mut difference = left[3].abs_diff(right[3]) as u16;
    for channel in 0..3 {
        let a = left[channel] as u16 * left[3] as u16 / 255;
        let b = right[channel] as u16 * right[3] as u16 / 255;
        difference = difference.max(a.abs_diff(b));
    }
    difference as u8
}

fn flood_mask(doc: &Document, x: i32, y: i32, tolerance: u8, global: bool) -> Vec<u8> {
    let width = doc.width as usize;
    let height = doc.height as usize;
    let mut mask = vec![0; width * height];
    if x < 0 || y < 0 || x >= doc.width as i32 || y >= doc.height as i32 {
        return mask;
    }
    let start = y as usize * width + x as usize;
    let pixels = doc.active_pixels();
    let seed = pixel_at(pixels, start);
    if global {
        for (index, value) in mask.iter_mut().enumerate() {
            if color_distance(pixel_at(pixels, index), seed) <= tolerance {
                *value = 255;
            }
        }
        return mask;
    }
    let mut queue = VecDeque::new();
    mask[start] = 255;
    queue.push_back(start);
    while let Some(index) = queue.pop_front() {
        let x = index % width;
        let y = index / width;
        let neighbors = [
            (x > 0).then(|| index - 1),
            (x + 1 < width).then_some(index + 1),
            (y > 0).then(|| index - width),
            (y + 1 < height).then_some(index + width),
        ];
        for next in neighbors.into_iter().flatten() {
            if mask[next] != 0 {
                continue;
            }
            if color_distance(pixel_at(pixels, next), seed) <= tolerance {
                mask[next] = 255;
                queue.push_back(next);
            } else {
                mask[next] = 1;
            }
        }
    }
    for value in &mut mask {
        if *value != 255 {
            *value = 0;
        }
    }
    mask
}

pub fn flood_fill(
    doc: &mut Document,
    x: i32,
    y: i32,
    color: Pixel,
    settings: &ToolSettings,
    global: bool,
) {
    let mask = flood_mask(doc, x, y, settings.tolerance, global);
    for (index, &value) in mask.iter().enumerate() {
        if value != 0 {
            put_pixel(
                doc,
                (index % doc.width as usize) as i32,
                (index / doc.width as usize) as i32,
                color,
                settings.opacity as f32 / 255.0,
                false,
            );
        }
    }
}

fn combine_selection(doc: &mut Document, mask: Vec<u8>, mode: SelectionMode) {
    if mode == SelectionMode::Replace {
        doc.selection = Some(mask);
        return;
    }
    let initial = if matches!(mode, SelectionMode::Subtract | SelectionMode::Intersect) {
        255
    } else {
        0
    };
    let existing = doc
        .selection
        .get_or_insert_with(|| vec![initial; mask.len()]);
    for (old, new) in existing.iter_mut().zip(mask) {
        *old = match mode {
            SelectionMode::Replace => new,
            SelectionMode::Union => (*old).max(new),
            SelectionMode::Subtract => old.saturating_sub(new),
            SelectionMode::Intersect => (*old).min(new),
            SelectionMode::Xor => old.abs_diff(new),
        };
    }
}

pub fn magic_wand(
    doc: &mut Document,
    x: i32,
    y: i32,
    settings: &ToolSettings,
    mode: SelectionMode,
    global: bool,
) {
    let mask = flood_mask(doc, x, y, settings.tolerance, global);
    combine_selection(doc, mask, mode);
}

pub fn select_rect(
    doc: &mut Document,
    start: (i32, i32),
    end: (i32, i32),
    ellipse: bool,
    mode: SelectionMode,
) {
    let left = start.0.min(end.0).max(0);
    let top = start.1.min(end.1).max(0);
    let right = (start.0.max(end.0) + 1).min(doc.width as i32);
    let bottom = (start.1.max(end.1) + 1).min(doc.height as i32);
    let center = (
        (start.0 as f32 + end.0 as f32 + 1.0) / 2.0,
        (start.1 as f32 + end.1 as f32 + 1.0) / 2.0,
    );
    let rx = ((end.0 as f32 - start.0 as f32).abs() + 1.0) / 2.0;
    let ry = ((end.1 as f32 - start.1 as f32).abs() + 1.0) / 2.0;
    let mut mask = vec![0; doc.width as usize * doc.height as usize];
    for y in top..bottom {
        for x in left..right {
            if !ellipse
                || ((x as f32 + 0.5 - center.0) / rx).powi(2)
                    + ((y as f32 + 0.5 - center.1) / ry).powi(2)
                    <= 1.0
            {
                mask[y as usize * doc.width as usize + x as usize] = 255;
            }
        }
    }
    combine_selection(doc, mask, mode);
}

pub fn select_lasso(doc: &mut Document, points: &[(i32, i32)], mode: SelectionMode) {
    if points.len() < 3 {
        return;
    }
    let left = points.iter().map(|p| p.0).min().unwrap_or(0).max(0);
    let right = points
        .iter()
        .map(|p| p.0)
        .max()
        .unwrap_or(0)
        .min(doc.width as i32 - 1);
    let top = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
    let bottom = points
        .iter()
        .map(|p| p.1)
        .max()
        .unwrap_or(0)
        .min(doc.height as i32 - 1);
    let mut mask = vec![0; doc.width as usize * doc.height as usize];
    for y in top..=bottom {
        for x in left..=right {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut inside = false;
            let mut previous = points[points.len() - 1];
            for &point in points {
                let (x1, y1) = (point.0 as f32, point.1 as f32);
                let (x2, y2) = (previous.0 as f32, previous.1 as f32);
                if (y1 > py) != (y2 > py) && px < (x2 - x1) * (py - y1) / (y2 - y1) + x1 {
                    inside = !inside;
                }
                previous = point;
            }
            if inside {
                mask[y as usize * doc.width as usize + x as usize] = 255;
            }
        }
    }
    combine_selection(doc, mask, mode);
}

pub fn invert_selection(doc: &mut Document) {
    if let Some(mask) = &mut doc.selection {
        for value in mask {
            *value = 255 - *value;
        }
    } else {
        doc.selection = Some(vec![255; doc.width as usize * doc.height as usize]);
    }
}

pub fn delete_selection(doc: &mut Document) {
    let selection = doc.selection.as_deref();
    let pixels = &mut doc.layers[doc.active_layer].pixels;
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        let amount = selection.map_or(255, |mask| mask[index]);
        pixel[3] = ((pixel[3] as u16 * (255 - amount) as u16 + 127) / 255) as u8;
        if pixel[3] == 0 {
            pixel.fill(0);
        }
    }
}

fn shape_distance(point: (f32, f32), center: (f32, f32), half: (f32, f32), shape: Tool) -> f32 {
    let x = (point.0 - center.0).abs();
    let y = (point.1 - center.1).abs();
    if shape == Tool::Ellipse {
        let rx = half.0.max(0.5);
        let ry = half.1.max(0.5);
        let k0 = (x / rx).hypot(y / ry);
        let k1 = (x / (rx * rx)).hypot(y / (ry * ry));
        return if k1 > 0.0 {
            k0 * (k0 - 1.0) / k1
        } else {
            -rx.min(ry)
        };
    }
    let rounding = if shape == Tool::RoundedRectangle {
        half.0.min(half.1).min(16.0) * 0.5
    } else {
        0.0
    };
    let qx = x - half.0 + rounding;
    let qy = y - half.1 + rounding;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - rounding
}

pub fn draw_shape(
    doc: &mut Document,
    start: (f32, f32),
    end: (f32, f32),
    color: Pixel,
    secondary: Pixel,
    settings: &ToolSettings,
    shape: Tool,
) {
    if shape == Tool::Line {
        stroke(doc, start, end, color, settings, false);
        return;
    }
    let center = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    let half = ((end.0 - start.0).abs() / 2.0, (end.1 - start.1).abs() / 2.0);
    let border = settings.size.clamp(1.0, 2048.0) / 2.0;
    let (left, top, right, bottom) = stroke_bounds(doc, start, end, border);
    for y in top..bottom {
        for x in left..right {
            let distance = shape_distance((x as f32 + 0.5, y as f32 + 0.5), center, half, shape);
            if settings.filled {
                let fill = if settings.outlined { secondary } else { color };
                put_pixel(
                    doc,
                    x,
                    y,
                    fill,
                    (0.5 - distance).clamp(0.0, 1.0) * settings.opacity as f32 / 255.0,
                    false,
                );
            }
            if settings.outlined {
                put_pixel(
                    doc,
                    x,
                    y,
                    color,
                    (border + 0.5 - distance.abs()).clamp(0.0, 1.0) * settings.opacity as f32
                        / 255.0,
                    false,
                );
            }
        }
    }
}

pub fn gradient(
    doc: &mut Document,
    start: (f32, f32),
    end: (f32, f32),
    primary: Pixel,
    secondary: Pixel,
    settings: &ToolSettings,
) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length = dx.hypot(dy).max(0.001);
    let angle = dy.atan2(dx);
    for y in 0..doc.height as i32 {
        for x in 0..doc.width as i32 {
            let px = x as f32 + 0.5 - start.0;
            let py = y as f32 + 0.5 - start.1;
            let along = (px * dx + py * dy) / (length * length);
            let across = (py * dx - px * dy) / (length * length);
            let t = match settings.gradient {
                GradientKind::Linear => along,
                GradientKind::Reflected => along.abs(),
                GradientKind::Radial => px.hypot(py) / length,
                GradientKind::Diamond => along.abs() + across.abs(),
                GradientKind::Conical => {
                    (py.atan2(px) - angle).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU
                }
            }
            .clamp(0.0, 1.0);
            let mut color = [0; 4];
            // interpolate premultiplied color so transparent stops do not leave dark fringes.
            let alpha = primary[3] as f32 * (1.0 - t) + secondary[3] as f32 * t;
            color[3] = alpha.round() as u8;
            if alpha > 0.0 {
                for channel in 0..3 {
                    color[channel] = ((primary[channel] as f32 * primary[3] as f32 * (1.0 - t)
                        + secondary[channel] as f32 * secondary[3] as f32 * t)
                        / alpha)
                        .round() as u8;
                }
            }
            put_pixel(doc, x, y, color, settings.opacity as f32 / 255.0, false);
        }
    }
}

pub fn clone_stroke(
    doc: &mut Document,
    source: &[u8],
    from: (f32, f32),
    to: (f32, f32),
    offset: (i32, i32),
    settings: &ToolSettings,
) {
    if source.len() != doc.active_pixels().len() {
        return;
    }
    let radius = settings.size.clamp(1.0, 2048.0) / 2.0;
    let (left, top, right, bottom) = stroke_bounds(doc, from, to, radius);
    for y in top..bottom {
        for x in left..right {
            let sx = x + offset.0;
            let sy = y + offset.1;
            if sx < 0 || sy < 0 || sx >= doc.width as i32 || sy >= doc.height as i32 {
                continue;
            }
            let distance = distance_to_line((x as f32 + 0.5, y as f32 + 0.5), from, to);
            let coverage = brush_coverage(distance, radius, settings.hardness)
                * settings.opacity as f32
                / 255.0;
            let color = pixel_at(source, sy as usize * doc.width as usize + sx as usize);
            put_pixel(doc, x, y, color, coverage, false);
        }
    }
}

pub fn recolor_stroke(
    doc: &mut Document,
    from: (f32, f32),
    to: (f32, f32),
    primary: Pixel,
    secondary: Pixel,
    settings: &ToolSettings,
) {
    let radius = settings.size.clamp(1.0, 2048.0) / 2.0;
    let (left, top, right, bottom) = stroke_bounds(doc, from, to, radius);
    for y in top..bottom {
        for x in left..right {
            let index = y as usize * doc.width as usize + x as usize;
            let old = pixel_at(doc.active_pixels(), index);
            if color_distance(old, secondary) > settings.tolerance {
                continue;
            }
            let distance = distance_to_line((x as f32 + 0.5, y as f32 + 0.5), from, to);
            let coverage = brush_coverage(distance, radius, settings.hardness)
                * settings.opacity as f32
                / 255.0;
            let selected = doc.selection.as_ref().map_or(255, |mask| mask[index]) as f32 / 255.0;
            let amount = coverage * selected;
            let mut replaced = old;
            for channel in 0..3 {
                replaced[channel] = (old[channel] as f32 * (1.0 - amount)
                    + primary[channel] as f32 * amount)
                    .round() as u8;
            }
            doc.active_pixels_mut()[index * 4..index * 4 + 4].copy_from_slice(&replaced);
        }
    }
}

pub fn move_pixels(
    doc: &mut Document,
    source: &[u8],
    source_selection: Option<&[u8]>,
    dx: i32,
    dy: i32,
    copy: bool,
) {
    let count = doc.width as usize * doc.height as usize;
    if source.len() != count * 4 || source_selection.is_some_and(|mask| mask.len() != count) {
        return;
    }
    let width = doc.width as usize;
    let height = doc.height as usize;
    let pixels = doc.active_pixels_mut();
    pixels.copy_from_slice(source);
    if !copy && dx == 0 && dy == 0 {
        doc.selection = source_selection.map(<[u8]>::to_vec);
        return;
    }
    if !copy {
        for index in 0..count {
            let selected = source_selection.map_or(255, |mask| mask[index]);
            pixels[index * 4 + 3] =
                ((pixels[index * 4 + 3] as u16 * (255 - selected) as u16 + 127) / 255) as u8;
            if pixels[index * 4 + 3] == 0 {
                pixels[index * 4..index * 4 + 4].fill(0);
            }
        }
    }
    for index in 0..count {
        let selected = source_selection.map_or(255, |mask| mask[index]);
        if selected == 0 {
            continue;
        }
        let tx = index as i32 % width as i32 + dx;
        let ty = index as i32 / width as i32 + dy;
        if tx < 0 || ty < 0 || tx >= width as i32 || ty >= height as i32 {
            continue;
        }
        let target = ty as usize * width + tx as usize;
        let color = pixel_at(source, index);
        let result = composite_pixel(pixel_at(pixels, target), color, selected, BlendMode::Normal);
        pixels[target * 4..target * 4 + 4].copy_from_slice(&result);
    }
    move_selection(doc, source_selection, dx, dy);
}

pub fn move_selection(doc: &mut Document, source: Option<&[u8]>, dx: i32, dy: i32) {
    let count = doc.width as usize * doc.height as usize;
    if source.is_some_and(|mask| mask.len() != count) {
        return;
    }
    if dx == 0 && dy == 0 {
        doc.selection = source.map(<[u8]>::to_vec);
        return;
    }
    let mut target = vec![0; count];
    for index in 0..count {
        let x = index as i32 % doc.width as i32 + dx;
        let y = index as i32 / doc.width as i32 + dy;
        if x >= 0 && y >= 0 && x < doc.width as i32 && y < doc.height as i32 {
            target[y as usize * doc.width as usize + x as usize] =
                source.map_or(255, |mask| mask[index]);
        }
    }
    doc.selection = Some(target);
}

fn transformed_sample(
    pixels: Option<&[u8]>,
    selection: Option<&[u8]>,
    width: u32,
    height: u32,
    point: (f64, f64),
) -> (Pixel, u8) {
    let x = point.0 - 0.5;
    let y = point.1 - 0.5;
    if x <= -1.0 || y <= -1.0 || x >= width as f64 || y >= height as f64 {
        return ([0; 4], 0);
    }
    let left = x.floor() as i32;
    let top = y.floor() as i32;
    let fx = x - left as f64;
    let fy = y - top as f64;
    let mut coverage = 0.0;
    let mut premultiplied = [0.0; 4];
    for row in 0..2 {
        for column in 0..2 {
            let sx = left + column;
            let sy = top + row;
            if sx < 0 || sy < 0 || sx >= width as i32 || sy >= height as i32 {
                continue;
            }
            let index = sy as usize * width as usize + sx as usize;
            let weight =
                if column == 0 { 1.0 - fx } else { fx } * if row == 0 { 1.0 - fy } else { fy };
            let selected = selection.map_or(255, |mask| mask[index]) as f64;
            coverage += selected * weight;
            if let Some(pixels) = pixels {
                let color = pixel_at(pixels, index);
                let alpha = color[3] as f64 * selected / 255.0 * weight;
                premultiplied[3] += alpha;
                for channel in 0..3 {
                    premultiplied[channel] += color[channel] as f64 * alpha;
                }
            }
        }
    }
    let mut color = [0; 4];
    color[3] = premultiplied[3].round() as u8;
    if color[3] > 0 {
        for channel in 0..3 {
            color[channel] = (premultiplied[channel] / premultiplied[3]).round() as u8;
        }
    }
    (color, coverage.round() as u8)
}

pub fn transform_pixels(
    doc: &mut Document,
    source: &[u8],
    source_selection: Option<&[u8]>,
    transform: &SelectionTransform,
    copy: bool,
) {
    let width = doc.width;
    let height = doc.height;
    let count = width as usize * height as usize;
    if !transform.valid()
        || source.len() != count * 4
        || source_selection.is_some_and(|mask| mask.len() != count)
    {
        return;
    }
    let pixels = doc.active_pixels_mut();
    pixels.copy_from_slice(source);
    if !copy && transform.identity() {
        doc.selection = source_selection.map(<[u8]>::to_vec);
        return;
    }
    if !copy {
        for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            let selected = source_selection.map_or(255, |mask| mask[index]);
            pixel[3] = ((pixel[3] as u16 * (255 - selected) as u16 + 127) / 255) as u8;
            if pixel[3] == 0 {
                pixel.fill(0);
            }
        }
    }
    let rotation = transform.rotation();
    let mut selection = vec![0; count];
    for y in 0..height {
        for x in 0..width {
            let index = y as usize * width as usize + x as usize;
            let (color, coverage) = transformed_sample(
                Some(source),
                source_selection,
                width,
                height,
                transform.source_point(x, y, rotation),
            );
            if color[3] != 0 {
                let blended =
                    composite_pixel(pixel_at(pixels, index), color, 255, BlendMode::Normal);
                pixels[index * 4..index * 4 + 4].copy_from_slice(&blended);
            }
            selection[index] = coverage;
        }
    }
    doc.selection = Some(selection);
}

pub fn transform_selection(
    doc: &mut Document,
    source_selection: Option<&[u8]>,
    transform: &SelectionTransform,
) {
    let count = doc.width as usize * doc.height as usize;
    if !transform.valid() || source_selection.is_some_and(|mask| mask.len() != count) {
        return;
    }
    if transform.identity() {
        doc.selection = source_selection.map(<[u8]>::to_vec);
        return;
    }
    let rotation = transform.rotation();
    let mut selection = vec![0; count];
    for y in 0..doc.height {
        for x in 0..doc.width {
            selection[y as usize * doc.width as usize + x as usize] = transformed_sample(
                None,
                source_selection,
                doc.width,
                doc.height,
                transform.source_point(x, y, rotation),
            )
            .1;
        }
    }
    doc.selection = Some(selection);
}

pub fn text(
    doc: &mut Document,
    position: (i32, i32),
    text: &str,
    size: f32,
    color: Pixel,
) -> Result<(), String> {
    static FONT: OnceLock<Result<fontdue::Font, String>> = OnceLock::new();
    let font = FONT
        .get_or_init(|| {
            fontdue::Font::from_bytes(
                include_bytes!("../assets/DejaVuSans.ttf") as &[u8],
                fontdue::FontSettings::default(),
            )
            .map_err(str::to_string)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    if !size.is_finite() || !(1.0..=512.0).contains(&size) {
        return Err("text size must be between 1 and 512 pixels".into());
    }
    if text.chars().count() > 16_384 {
        return Err("text is limited to 16384 characters".into());
    }
    let line_metrics = font
        .horizontal_line_metrics(size)
        .ok_or("font has no horizontal metrics")?;
    let mut pen_x = position.0 as f32;
    let mut baseline = position.1 as f32 + line_metrics.ascent;
    let mut previous = None;
    for character in text.chars() {
        if character == '\n' {
            pen_x = position.0 as f32;
            baseline += line_metrics.new_line_size;
            previous = None;
            continue;
        }
        if character == '\r' {
            continue;
        }
        if character == '\t' {
            pen_x += size * 2.0;
            previous = None;
            continue;
        }
        if let Some(left) = previous {
            pen_x += font.horizontal_kern(left, character, size).unwrap_or(0.0);
        }
        let (metrics, bitmap) = font.rasterize(character, size);
        let left = pen_x.round() as i32 + metrics.xmin;
        let top = baseline.round() as i32 - metrics.ymin - metrics.height as i32;
        for y in 0..metrics.height {
            for x in 0..metrics.width {
                put_pixel(
                    doc,
                    left + x as i32,
                    top + y as i32,
                    color,
                    bitmap[y * metrics.width + x] as f32 / 255.0,
                    false,
                );
            }
        }
        pen_x += metrics.advance_width;
        previous = Some(character);
    }
    Ok(())
}

pub fn selected_pixels(doc: &Document) -> (u32, u32, Vec<u8>) {
    let Some((x, y, width, height)) = doc.selection_bounds().or_else(|| {
        doc.selection
            .is_none()
            .then_some((0, 0, doc.width, doc.height))
    }) else {
        return (0, 0, Vec::new());
    };
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for row in 0..height as usize {
        for column in 0..width as usize {
            let source = (y as usize + row) * doc.width as usize + x as usize + column;
            let target = row * width as usize + column;
            let mut color = pixel_at(doc.active_pixels(), source);
            color[3] = ((color[3] as u16
                * doc.selection.as_ref().map_or(255, |mask| mask[source]) as u16
                + 127)
                / 255) as u8;
            if color[3] == 0 {
                color = [0; 4];
            }
            pixels[target * 4..target * 4 + 4].copy_from_slice(&color);
        }
    }
    (width, height, pixels)
}

pub fn paste(
    doc: &mut Document,
    width: u32,
    height: u32,
    pixels: &[u8],
    position: (i32, i32),
) -> Result<(), String> {
    let length = crate::document::validate_dimensions(width, height)?;
    if pixels.len() != length {
        return Err("clipboard pixels have invalid dimensions".into());
    }
    let mut selection = vec![0; doc.width as usize * doc.height as usize];
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let tx = position.0 + x;
            let ty = position.1 + y;
            if tx < 0 || ty < 0 || tx >= doc.width as i32 || ty >= doc.height as i32 {
                continue;
            }
            let target = ty as usize * doc.width as usize + tx as usize;
            let source = y as usize * width as usize + x as usize;
            let color = composite_pixel(
                pixel_at(doc.active_pixels(), target),
                pixel_at(pixels, source),
                255,
                BlendMode::Normal,
            );
            doc.active_pixels_mut()[target * 4..target * 4 + 4].copy_from_slice(&color);
            selection[target] = 255;
        }
    }
    doc.selection = Some(selection);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transparent(width: u32, height: u32) -> Document {
        Document::from_rgba(
            width,
            height,
            vec![0; width as usize * height as usize * 4],
            "test".into(),
        )
        .unwrap()
    }

    #[test]
    fn flood_stops_at_boundary_and_selection() {
        let mut doc = Document::new(5, 1).unwrap();
        doc.active_pixels_mut()[8..12].copy_from_slice(&[0, 0, 0, 255]);
        doc.selection = Some(vec![255, 0, 255, 255, 255]);
        let settings = ToolSettings {
            tolerance: 0,
            ..Default::default()
        };
        flood_fill(&mut doc, 0, 0, [255, 0, 0, 255], &settings, false);
        assert_eq!(pixel_at(doc.active_pixels(), 0), [255, 0, 0, 255]);
        assert_eq!(pixel_at(doc.active_pixels(), 1), [255; 4]);
        assert_eq!(pixel_at(doc.active_pixels(), 3), [255; 4]);
        flood_fill(&mut doc, 3, 0, [0, 0, 255, 255], &settings, true);
        assert_eq!(pixel_at(doc.active_pixels(), 4), [0, 0, 255, 255]);
        assert_eq!(pixel_at(doc.active_pixels(), 2), [0, 0, 0, 255]);
    }

    #[test]
    fn eraser_removes_alpha_not_rgb_paint() {
        let mut doc = Document::new(3, 1).unwrap();
        let settings = ToolSettings {
            size: 1.0,
            opacity: 128,
            ..Default::default()
        };
        stroke(
            &mut doc,
            (1.5, 0.5),
            (1.5, 0.5),
            [0, 0, 0, 255],
            &settings,
            true,
        );
        assert_eq!(pixel_at(doc.active_pixels(), 1), [255, 255, 255, 127]);
        assert_eq!(pixel_at(doc.active_pixels(), 0), [255; 4]);
    }

    #[test]
    fn move_cuts_source_and_preserves_other_pixels() {
        let mut doc = transparent(4, 1);
        doc.active_pixels_mut()[0..4].copy_from_slice(&[20, 40, 60, 255]);
        doc.active_pixels_mut()[12..16].copy_from_slice(&[80, 90, 100, 255]);
        let source = doc.active_pixels().to_vec();
        move_pixels(&mut doc, &source, Some(&[255, 0, 0, 0]), 1, 0, false);
        assert_eq!(pixel_at(doc.active_pixels(), 0), [0; 4]);
        assert_eq!(pixel_at(doc.active_pixels(), 1), [20, 40, 60, 255]);
        assert_eq!(pixel_at(doc.active_pixels(), 3), [80, 90, 100, 255]);
        assert_eq!(doc.selection.as_deref(), Some([0, 255, 0, 0].as_slice()));
    }

    #[test]
    fn overlapping_move_reads_drag_start() {
        let mut doc = transparent(3, 1);
        doc.active_pixels_mut()
            .copy_from_slice(&[1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255]);
        let source = doc.active_pixels().to_vec();
        move_pixels(&mut doc, &source, None, 1, 0, false);
        assert_eq!(
            doc.active_pixels(),
            &[0, 0, 0, 0, 1, 2, 3, 255, 4, 5, 6, 255]
        );
    }

    #[test]
    fn paste_clips_to_canvas_and_tracks_selection() {
        let mut doc = transparent(2, 2);
        paste(&mut doc, 2, 2, &[255; 16], (-1, -1)).unwrap();
        assert_eq!(pixel_at(doc.active_pixels(), 0), [255; 4]);
        assert_eq!(pixel_at(doc.active_pixels(), 1), [0; 4]);
        assert_eq!(doc.selection.as_deref(), Some([255, 0, 0, 0].as_slice()));
    }

    #[test]
    fn selection_combination_and_clipboard_alpha() {
        let mut doc = Document::new(4, 1).unwrap();
        select_rect(&mut doc, (0, 0), (1, 0), false, SelectionMode::Replace);
        select_rect(&mut doc, (1, 0), (2, 0), false, SelectionMode::Xor);
        assert_eq!(doc.selection.as_deref(), Some([255, 0, 255, 0].as_slice()));
        let (width, height, pixels) = selected_pixels(&doc);
        assert_eq!((width, height), (3, 1));
        assert_eq!(pixel_at(&pixels, 1), [0; 4]);
    }

    #[test]
    fn continuous_stroke_has_no_gaps() {
        let mut doc = transparent(20, 1);
        let settings = ToolSettings {
            size: 1.0,
            ..Default::default()
        };
        stroke(
            &mut doc,
            (0.5, 0.5),
            (19.5, 0.5),
            [3, 6, 9, 255],
            &settings,
            false,
        );
        assert!(doc
            .active_pixels()
            .chunks_exact(4)
            .all(|pixel| pixel == [3, 6, 9, 255]));
    }

    #[test]
    fn text_rasterizes_and_obeys_selection() {
        let mut doc = transparent(100, 50);
        doc.selection = Some(
            (0..5000)
                .map(|i| if i % 100 < 50 { 255 } else { 0 })
                .collect(),
        );
        text(&mut doc, (0, 0), "test", 32.0, [20, 40, 60, 255]).unwrap();
        assert!(doc
            .active_pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0));
        assert!(doc
            .active_pixels()
            .chunks_exact(4)
            .enumerate()
            .filter(|(i, _)| i % 100 >= 50)
            .all(|(_, pixel)| pixel[3] == 0));
    }

    #[test]
    fn gradient_interpolates_transparency_without_dark_fringe() {
        let mut doc = transparent(3, 1);
        gradient(
            &mut doc,
            (0.5, 0.5),
            (2.5, 0.5),
            [255, 0, 0, 255],
            [0, 0, 0, 0],
            &ToolSettings::default(),
        );
        assert_eq!(pixel_at(doc.active_pixels(), 1), [255, 0, 0, 128]);
    }

    #[test]
    fn identity_transform_and_move_preserve_partial_alpha() {
        let mut doc = transparent(2, 1);
        let source = [10, 20, 30, 123, 80, 90, 100, 0];
        let selection = [128, 0];
        transform_pixels(
            &mut doc,
            &source,
            Some(&selection),
            &SelectionTransform::default(),
            false,
        );
        assert_eq!(doc.active_pixels(), source);
        assert_eq!(doc.selection.as_deref(), Some(selection.as_slice()));
        move_pixels(&mut doc, &source, Some(&selection), 0, 0, false);
        assert_eq!(doc.active_pixels(), source);
        assert_eq!(doc.selection.as_deref(), Some(selection.as_slice()));
        transform_pixels(
            &mut doc,
            &source,
            None,
            &SelectionTransform::default(),
            false,
        );
        assert_eq!(doc.active_pixels(), source);
        assert!(doc.selection.is_none());
    }

    #[test]
    fn quarter_turn_maps_pixels_and_rectangular_selection_exactly() {
        let mut doc = transparent(3, 3);
        doc.active_pixels_mut()[0..8].copy_from_slice(&[200, 30, 40, 128, 50, 180, 70, 255]);
        let source = doc.active_pixels().to_vec();
        let mask = [255, 255, 0, 0, 0, 0, 0, 0, 0];
        let transform = SelectionTransform {
            center: (1.5, 1.5),
            angle: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        transform_pixels(&mut doc, &source, Some(&mask), &transform, false);
        assert_eq!(pixel_at(doc.active_pixels(), 2), [200, 30, 40, 128]);
        assert_eq!(pixel_at(doc.active_pixels(), 5), [50, 180, 70, 255]);
        assert_eq!(pixel_at(doc.active_pixels(), 0), [0; 4]);
        assert_eq!(
            doc.selection.as_deref(),
            Some([0, 0, 255, 0, 0, 255, 0, 0, 0].as_slice())
        );
    }

    #[test]
    fn scaling_clips_and_interpolates_premultiplied_selected_color() {
        let mut doc = transparent(2, 1);
        let source = [255, 0, 0, 255, 0, 255, 0, 0];
        let transform = SelectionTransform {
            center: (0.5, 0.5),
            scale: (2.0, 1.0),
            ..Default::default()
        };
        transform_pixels(&mut doc, &source, Some(&[255, 0]), &transform, false);
        assert_eq!(doc.active_pixels(), [255, 0, 0, 255, 255, 0, 0, 128]);
        assert_eq!(doc.selection.as_deref(), Some([255, 128].as_slice()));
        assert_eq!(doc.active_pixels().len(), 8);
    }

    #[test]
    fn transform_preserves_unselected_background() {
        let mut doc = transparent(4, 1);
        let source = [255, 0, 0, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255];
        let transform = SelectionTransform {
            translation: (1.0, 0.0),
            ..Default::default()
        };
        transform_pixels(&mut doc, &source, Some(&[255, 0, 0, 0]), &transform, false);
        assert_eq!(pixel_at(doc.active_pixels(), 0), [0; 4]);
        assert_eq!(pixel_at(doc.active_pixels(), 1), [255, 0, 0, 128]);
        assert_eq!(pixel_at(doc.active_pixels(), 3), [0, 0, 255, 255]);
    }

    #[test]
    fn mask_transform_does_not_change_layer_pixels() {
        let mut doc = Document::new(3, 3).unwrap();
        let source = doc.active_pixels().to_vec();
        let mask = [255, 255, 0, 0, 0, 0, 0, 0, 0];
        let transform = SelectionTransform {
            center: (1.5, 1.5),
            angle: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        };
        transform_selection(&mut doc, Some(&mask), &transform);
        assert_eq!(doc.active_pixels(), source);
        assert_eq!(
            doc.selection.as_deref(),
            Some([0, 0, 255, 0, 0, 255, 0, 0, 0].as_slice())
        );
    }

    #[test]
    fn invalid_transforms_leave_pixels_and_selection_unchanged() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.selection = Some(vec![255, 0, 0, 0]);
        let source = vec![0; 16];
        for scale in [(0.0, 1.0), (f32::NAN, 1.0), (1.0, f32::INFINITY)] {
            let transform = SelectionTransform {
                scale,
                ..Default::default()
            };
            transform_pixels(&mut doc, &source, None, &transform, false);
            transform_selection(&mut doc, None, &transform);
            assert_eq!(doc.active_pixels(), &[255; 16]);
            assert_eq!(doc.selection.as_deref(), Some([255, 0, 0, 0].as_slice()));
        }
    }
}
