use std::path::PathBuf;

pub type Pixel = [u8; 4];

pub const MAX_DIMENSION: u32 = 8_192;
pub const MAX_PIXELS: usize = 67_108_864;
pub const MAX_LAYERS: usize = 256;
const MAX_DOCUMENT_BYTES: usize = 256 * 1024 * 1024;
const MAX_HISTORY_BYTES: usize = 256 * 1024 * 1024;
const MAX_HISTORY_ENTRIES: usize = 128;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Additive,
    ColorBurn,
    ColorDodge,
    Reflect,
    Glow,
    Overlay,
    Difference,
    Negation,
    Lighten,
    Darken,
    Screen,
    Xor,
}

impl BlendMode {
    pub const ALL: [Self; 14] = [
        Self::Normal,
        Self::Multiply,
        Self::Additive,
        Self::ColorBurn,
        Self::ColorDodge,
        Self::Reflect,
        Self::Glow,
        Self::Overlay,
        Self::Difference,
        Self::Negation,
        Self::Lighten,
        Self::Darken,
        Self::Screen,
        Self::Xor,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Additive => "Additive",
            Self::ColorBurn => "Color Burn",
            Self::ColorDodge => "Color Dodge",
            Self::Reflect => "Reflect",
            Self::Glow => "Glow",
            Self::Overlay => "Overlay",
            Self::Difference => "Difference",
            Self::Negation => "Negation",
            Self::Lighten => "Lighten",
            Self::Darken => "Darken",
            Self::Screen => "Screen",
            Self::Xor => "Xor",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend_mode: BlendMode,
    pub pixels: Vec<u8>,
}

impl Layer {
    pub fn new(name: impl Into<String>, pixels: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            visible: true,
            opacity: 255,
            blend_mode: BlendMode::Normal,
            pixels,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    width: u32,
    height: u32,
    layers: Vec<Layer>,
    active_layer: usize,
    selection: Option<Vec<u8>>,
}

impl Snapshot {
    fn content_eq(&self, other: &Self) -> bool {
        self.width == other.width && self.height == other.height && self.layers == other.layers
    }

    fn size(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| layer.pixels.len())
            .sum::<usize>()
            + self.selection.as_ref().map_or(0, Vec::len)
    }
}

#[derive(Clone, Debug)]
struct HistoryEntry {
    label: String,
    state: Snapshot,
    revision: u64,
}

#[derive(Clone, Debug)]
struct PendingEdit {
    label: String,
    before: Snapshot,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub layers: Vec<Layer>,
    pub active_layer: usize,
    pub selection: Option<Vec<u8>>,
    pub name: String,
    pub path: Option<PathBuf>,
    history: Vec<HistoryEntry>,
    history_position: usize,
    pending_edit: Option<PendingEdit>,
    next_revision: u64,
    saved_revision: u64,
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<usize, String> {
    if width == 0 || height == 0 {
        return Err("image dimensions must be greater than zero".into());
    }
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(format!(
            "image dimensions must not exceed {MAX_DIMENSION} pixels"
        ));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or("image dimensions are too large")?;
    if pixels > MAX_PIXELS {
        return Err("images larger than 64 megapixels are not supported yet".into());
    }
    Ok(pixels * 4)
}

impl Document {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        let length = validate_dimensions(width, height)?;
        Self::from_rgba(width, height, vec![255; length], "Untitled".into())
    }

    pub fn from_rgba(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        name: String,
    ) -> Result<Self, String> {
        Self::from_layers(
            width,
            height,
            vec![Layer::new("Background", pixels)],
            0,
            name,
        )
    }

    pub fn from_layers(
        width: u32,
        height: u32,
        layers: Vec<Layer>,
        active_layer: usize,
        name: String,
    ) -> Result<Self, String> {
        let mut document = Self {
            width,
            height,
            layers,
            active_layer,
            selection: None,
            name,
            path: None,
            history: Vec::new(),
            history_position: 0,
            pending_edit: None,
            next_revision: 1,
            saved_revision: 0,
        };
        document.validate()?;
        document.history.push(HistoryEntry {
            label: "Initial image".into(),
            state: document.snapshot(),
            revision: 0,
        });
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), String> {
        let length = validate_dimensions(self.width, self.height)?;
        if self.layers.is_empty() || self.active_layer >= self.layers.len() {
            return Err("the document must have an active layer".into());
        }
        check_document_size(length, self.layers.len())?;
        if self.layers.iter().any(|layer| layer.pixels.len() != length) {
            return Err("layer pixels do not match the document dimensions".into());
        }
        if self
            .selection
            .as_ref()
            .is_some_and(|mask| mask.len() != length / 4)
        {
            return Err("selection pixels do not match the document dimensions".into());
        }
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            width: self.width,
            height: self.height,
            layers: self.layers.clone(),
            active_layer: self.active_layer,
            selection: self.selection.clone(),
        }
    }

    fn restore(&mut self, state: Snapshot) {
        self.width = state.width;
        self.height = state.height;
        self.layers = state.layers;
        self.active_layer = state.active_layer;
        self.selection = state.selection;
    }

    pub fn begin_edit(&mut self, label: &str) {
        if self.pending_edit.is_none() {
            self.pending_edit = Some(PendingEdit {
                label: label.into(),
                before: self.snapshot(),
            });
        }
    }

    pub fn finish_edit(&mut self) -> bool {
        let Some(edit) = self.pending_edit.take() else {
            return false;
        };
        let state = self.snapshot();
        if state == edit.before {
            return false;
        }
        let revision = if state.content_eq(&edit.before) {
            self.history[self.history_position].revision
        } else {
            let revision = self.next_revision;
            self.next_revision += 1;
            revision
        };
        self.history.truncate(self.history_position + 1);
        self.history.push(HistoryEntry {
            label: edit.label,
            state,
            revision,
        });
        self.history_position = self.history.len() - 1;
        let mut history_size = self
            .history
            .iter()
            .map(|entry| entry.state.size())
            .sum::<usize>();
        while self.history.len() > 1
            && (history_size > MAX_HISTORY_BYTES || self.history.len() > MAX_HISTORY_ENTRIES)
        {
            history_size -= self.history.remove(0).state.size();
            self.history_position -= 1;
        }
        true
    }

    pub fn cancel_edit(&mut self) {
        if let Some(edit) = self.pending_edit.take() {
            self.restore(edit.before);
        }
    }

    pub fn undo(&mut self) -> bool {
        self.finish_edit();
        if self.history_position == 0 {
            return false;
        }
        self.history_position -= 1;
        self.restore(self.history[self.history_position].state.clone());
        true
    }

    pub fn redo(&mut self) -> bool {
        self.finish_edit();
        if self.history_position + 1 >= self.history.len() {
            return false;
        }
        self.history_position += 1;
        self.restore(self.history[self.history_position].state.clone());
        true
    }

    pub fn mark_saved(&mut self) {
        self.finish_edit();
        let state = self.snapshot();
        let entry = &mut self.history[self.history_position];
        if !entry.state.content_eq(&state) {
            entry.revision = self.next_revision;
            self.next_revision += 1;
        }
        entry.state = state;
        self.saved_revision = entry.revision;
    }

    pub fn is_dirty(&self) -> bool {
        let current = &self.history[self.history_position];
        if current.revision != self.saved_revision {
            return true;
        }
        self.width != current.state.width
            || self.height != current.state.height
            || self.layers != current.state.layers
    }

    pub fn mark_unsaved(&mut self) {
        self.saved_revision = u64::MAX;
    }

    pub fn history_labels(&self) -> Vec<String> {
        self.history
            .iter()
            .map(|entry| entry.label.clone())
            .collect()
    }

    pub fn history_position(&self) -> usize {
        self.history_position
    }

    pub fn can_undo(&self) -> bool {
        self.history_position > 0 || self.pending_edit.is_some()
    }

    pub fn can_redo(&self) -> bool {
        self.history_position + 1 < self.history.len()
    }

    pub fn active_pixels(&self) -> &[u8] {
        self.layers
            .get(self.active_layer)
            .map_or(&[], |layer| &layer.pixels)
    }

    pub fn active_pixels_mut(&mut self) -> &mut [u8] {
        self.layers
            .get_mut(self.active_layer)
            .map_or(&mut [], |layer| &mut layer.pixels)
    }

    pub fn composite(&self) -> Vec<u8> {
        let Ok(length) = validate_dimensions(self.width, self.height) else {
            return Vec::new();
        };
        let mut pixels = vec![0; length];
        for layer in &self.layers {
            if !layer.visible || layer.opacity == 0 || layer.pixels.len() != length {
                continue;
            }
            for (destination, source) in
                pixels.chunks_exact_mut(4).zip(layer.pixels.chunks_exact(4))
            {
                let result = composite_pixel(
                    [
                        destination[0],
                        destination[1],
                        destination[2],
                        destination[3],
                    ],
                    [source[0], source[1], source[2], source[3]],
                    layer.opacity,
                    layer.blend_mode,
                );
                destination.copy_from_slice(&result);
            }
        }
        pixels
    }

    pub fn add_layer(&mut self) -> Result<(), String> {
        self.validate()?;
        let length = validate_dimensions(self.width, self.height)?;
        check_document_size(length, self.layers.len() + 1)?;
        self.begin_edit("Add New Layer");
        let layer = Layer::new(format!("Layer {}", self.layers.len() + 1), vec![0; length]);
        self.active_layer += 1;
        self.layers.insert(self.active_layer, layer);
        self.finish_edit();
        Ok(())
    }

    pub fn duplicate_layer(&mut self) -> Result<(), String> {
        self.validate()?;
        check_document_size(self.layers[0].pixels.len(), self.layers.len() + 1)?;
        self.begin_edit("Duplicate Layer");
        let mut layer = self.layers[self.active_layer].clone();
        layer.name.push_str(" copy");
        self.active_layer += 1;
        self.layers.insert(self.active_layer, layer);
        self.finish_edit();
        Ok(())
    }

    pub fn delete_layer(&mut self) -> Result<(), String> {
        self.validate()?;
        if self.layers.len() == 1 {
            return Err("the last layer cannot be deleted".into());
        }
        self.begin_edit("Delete Layer");
        self.layers.remove(self.active_layer);
        self.active_layer = self.active_layer.min(self.layers.len() - 1);
        self.finish_edit();
        Ok(())
    }

    pub fn move_layer(&mut self, delta: i32) -> Result<(), String> {
        self.validate()?;
        let target = self.active_layer as i64 + i64::from(delta);
        if target < 0 || target >= self.layers.len() as i64 {
            return Err("the layer is already at the edge of the stack".into());
        }
        self.begin_edit("Move Layer");
        let layer = self.layers.remove(self.active_layer);
        self.active_layer = target as usize;
        self.layers.insert(self.active_layer, layer);
        self.finish_edit();
        Ok(())
    }

    pub fn merge_down(&mut self) -> Result<(), String> {
        self.validate()?;
        if self.active_layer == 0 {
            return Err("there is no layer below the active layer".into());
        }
        self.begin_edit("Merge Layer Down");
        let top = self.layers.remove(self.active_layer);
        self.active_layer -= 1;
        let bottom = &mut self.layers[self.active_layer];
        for (destination, source) in bottom
            .pixels
            .chunks_exact_mut(4)
            .zip(top.pixels.chunks_exact(4))
        {
            let base = if bottom.visible {
                composite_pixel(
                    [0; 4],
                    destination.try_into().unwrap(),
                    bottom.opacity,
                    BlendMode::Normal,
                )
            } else {
                [0; 4]
            };
            let result = if top.visible {
                composite_pixel(
                    base,
                    source.try_into().unwrap(),
                    top.opacity,
                    top.blend_mode,
                )
            } else {
                base
            };
            destination.copy_from_slice(&result);
        }
        bottom.opacity = 255;
        bottom.visible = true;
        bottom.blend_mode = BlendMode::Normal;
        self.finish_edit();
        Ok(())
    }

    pub fn flatten(&mut self) -> Result<(), String> {
        self.validate()?;
        let pixels = self.composite();
        self.begin_edit("Flatten");
        self.layers = vec![Layer::new("Background", pixels)];
        self.active_layer = 0;
        self.finish_edit();
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.validate()?;
        let length = validate_dimensions(width, height)?;
        check_document_size(length, self.layers.len())?;
        if self.width == width && self.height == height {
            return Ok(());
        }
        self.begin_edit("Resize");
        for layer in &mut self.layers {
            layer.pixels = resize_rgba(&layer.pixels, self.width, self.height, width, height);
        }
        if let Some(mask) = &mut self.selection {
            let old_mask = std::mem::replace(mask, vec![0; length / 4]);
            for y in 0..height {
                for x in 0..width {
                    let sx = (u64::from(x) * u64::from(self.width) / u64::from(width)) as u32;
                    let sy = (u64::from(y) * u64::from(self.height) / u64::from(height)) as u32;
                    mask[(y * width + x) as usize] = old_mask[(sy * self.width + sx) as usize];
                }
            }
        }
        self.width = width;
        self.height = height;
        self.finish_edit();
        Ok(())
    }

    pub fn canvas_size(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.validate()?;
        validate_dimensions(width, height)?;
        let offset_x = (i64::from(width) - i64::from(self.width)) / 2;
        let offset_y = (i64::from(height) - i64::from(self.height)) / 2;
        self.remap("Canvas Size", width, height, |x, y| {
            (i64::from(x) - offset_x, i64::from(y) - offset_y)
        })
    }

    pub fn selection_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let mask = self.selection.as_ref()?;
        let length = validate_dimensions(self.width, self.height).ok()?;
        if mask.len() != length / 4 {
            return None;
        }
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        for (index, &value) in mask.iter().enumerate() {
            if value != 0 {
                let x = index as u32 % self.width;
                let y = index as u32 / self.width;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
        if min_x > max_x || min_y > max_y {
            return None;
        }
        Some((min_x, min_y, max_x + 1 - min_x, max_y + 1 - min_y))
    }

    pub fn crop_selection(&mut self) -> Result<(), String> {
        let (x, y, width, height) = self
            .selection_bounds()
            .ok_or("select an area to crop first")?;
        self.remap("Crop to Selection", width, height, |dx, dy| {
            (i64::from(dx + x), i64::from(dy + y))
        })
    }

    pub fn flip_horizontal(&mut self) -> Result<(), String> {
        let width = i64::from(self.width);
        self.remap("Flip Horizontal", self.width, self.height, |x, y| {
            (width - 1 - i64::from(x), i64::from(y))
        })
    }

    pub fn flip_vertical(&mut self) -> Result<(), String> {
        let height = i64::from(self.height);
        self.remap("Flip Vertical", self.width, self.height, |x, y| {
            (i64::from(x), height - 1 - i64::from(y))
        })
    }

    pub fn rotate_cw(&mut self) -> Result<(), String> {
        let height = i64::from(self.height);
        self.remap(
            "Rotate 90 degrees Clockwise",
            self.height,
            self.width,
            |x, y| (i64::from(y), height - 1 - i64::from(x)),
        )
    }

    pub fn rotate_ccw(&mut self) -> Result<(), String> {
        let width = i64::from(self.width);
        self.remap(
            "Rotate 90 degrees Counterclockwise",
            self.height,
            self.width,
            |x, y| (width - 1 - i64::from(y), i64::from(x)),
        )
    }

    fn remap(
        &mut self,
        label: &str,
        width: u32,
        height: u32,
        source: impl Fn(u32, u32) -> (i64, i64),
    ) -> Result<(), String> {
        self.validate()?;
        let length = validate_dimensions(width, height)?;
        check_document_size(length, self.layers.len())?;
        self.begin_edit(label);
        for layer in &mut self.layers {
            let mut pixels = vec![0; length];
            for y in 0..height {
                for x in 0..width {
                    let (sx, sy) = source(x, y);
                    if sx >= 0
                        && sy >= 0
                        && sx < i64::from(self.width)
                        && sy < i64::from(self.height)
                    {
                        let old = (sy as usize * self.width as usize + sx as usize) * 4;
                        let new = (y as usize * width as usize + x as usize) * 4;
                        pixels[new..new + 4].copy_from_slice(&layer.pixels[old..old + 4]);
                    }
                }
            }
            layer.pixels = pixels;
        }
        if let Some(mask) = &mut self.selection {
            let old_mask = std::mem::replace(mask, vec![0; length / 4]);
            for y in 0..height {
                for x in 0..width {
                    let (sx, sy) = source(x, y);
                    if sx >= 0
                        && sy >= 0
                        && sx < i64::from(self.width)
                        && sy < i64::from(self.height)
                    {
                        mask[(y * width + x) as usize] =
                            old_mask[sy as usize * self.width as usize + sx as usize];
                    }
                }
            }
        }
        self.width = width;
        self.height = height;
        self.finish_edit();
        Ok(())
    }
}

fn check_document_size(layer_length: usize, layers: usize) -> Result<(), String> {
    if layers > MAX_LAYERS {
        return Err(format!("documents support at most {MAX_LAYERS} layers"));
    }
    if layer_length
        .checked_mul(layers)
        .is_none_or(|bytes| bytes > MAX_DOCUMENT_BYTES)
    {
        return Err("the document would exceed the 256 MiB layer limit".into());
    }
    Ok(())
}

pub fn composite_pixel(destination: Pixel, source: Pixel, opacity: u8, mode: BlendMode) -> Pixel {
    let source_alpha = f32::from(source[3]) * f32::from(opacity) / (255.0 * 255.0);
    if source_alpha == 0.0 {
        return destination;
    }
    let destination_alpha = f32::from(destination[3]) / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    let mut result = [0; 4];
    for channel in 0..3 {
        let d = f32::from(destination[channel]) / 255.0;
        let s = f32::from(source[channel]) / 255.0;
        let blended = blend_channel(d, s, mode);
        let premultiplied = source_alpha
            * ((1.0 - destination_alpha) * s + destination_alpha * blended)
            + destination_alpha * (1.0 - source_alpha) * d;
        result[channel] = (premultiplied / alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    result
}

fn blend_channel(destination: f32, source: f32, mode: BlendMode) -> f32 {
    match mode {
        BlendMode::Normal => source,
        BlendMode::Multiply => destination * source,
        BlendMode::Additive => (destination + source).min(1.0),
        BlendMode::ColorBurn => {
            if destination >= 1.0 {
                1.0
            } else if source <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - destination) / source).min(1.0)
            }
        }
        BlendMode::ColorDodge => {
            if destination <= 0.0 {
                0.0
            } else if source >= 1.0 {
                1.0
            } else {
                (destination / (1.0 - source)).min(1.0)
            }
        }
        BlendMode::Reflect => {
            if source >= 1.0 {
                1.0
            } else {
                (destination * destination / (1.0 - source)).min(1.0)
            }
        }
        BlendMode::Glow => {
            if destination >= 1.0 {
                1.0
            } else {
                (source * source / (1.0 - destination)).min(1.0)
            }
        }
        BlendMode::Overlay => {
            if destination <= 0.5 {
                2.0 * destination * source
            } else {
                1.0 - 2.0 * (1.0 - destination) * (1.0 - source)
            }
        }
        BlendMode::Difference => (destination - source).abs(),
        BlendMode::Negation => 1.0 - (1.0 - destination - source).abs(),
        BlendMode::Lighten => destination.max(source),
        BlendMode::Darken => destination.min(source),
        BlendMode::Screen => 1.0 - (1.0 - destination) * (1.0 - source),
        BlendMode::Xor => {
            ((destination * 255.0).round() as u8 ^ (source * 255.0).round() as u8) as f32 / 255.0
        }
    }
}

fn resize_rgba(pixels: &[u8], old_width: u32, old_height: u32, width: u32, height: u32) -> Vec<u8> {
    let mut output = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        let source_y = ((y as f32 + 0.5) * old_height as f32 / height as f32 - 0.5).max(0.0);
        let y0 = (source_y.floor() as u32).min(old_height - 1);
        let y1 = (y0 + 1).min(old_height - 1);
        let fy = source_y - source_y.floor();
        for x in 0..width {
            let source_x = ((x as f32 + 0.5) * old_width as f32 / width as f32 - 0.5).max(0.0);
            let x0 = (source_x.floor() as u32).min(old_width - 1);
            let x1 = (x0 + 1).min(old_width - 1);
            let fx = source_x - source_x.floor();
            let samples = [
                (x0, y0, (1.0 - fx) * (1.0 - fy)),
                (x1, y0, fx * (1.0 - fy)),
                (x0, y1, (1.0 - fx) * fy),
                (x1, y1, fx * fy),
            ];
            let mut accumulated = [0.0; 4];
            for (sx, sy, weight) in samples {
                let index = (sy * old_width + sx) as usize * 4;
                let alpha = f32::from(pixels[index + 3]) / 255.0;
                for channel in 0..3 {
                    accumulated[channel] += f32::from(pixels[index + channel]) * alpha * weight;
                }
                accumulated[3] += alpha * weight;
            }
            let index = (y * width + x) as usize * 4;
            if accumulated[3] > 0.0 {
                for channel in 0..3 {
                    output[index + channel] = (accumulated[channel] / accumulated[3])
                        .round()
                        .clamp(0.0, 255.0) as u8;
                }
            }
            output[index + 3] = (accumulated[3] * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_alpha_compositing_handles_transparent_backdrops() {
        assert_eq!(
            composite_pixel([0, 0, 0, 0], [200, 100, 50, 128], 255, BlendMode::Multiply),
            [200, 100, 50, 128]
        );
        assert_eq!(
            composite_pixel([0, 0, 255, 128], [255, 0, 0, 128], 255, BlendMode::Normal),
            [170, 0, 85, 192]
        );
        assert_eq!(
            composite_pixel([10, 20, 30, 255], [240, 0, 0, 255], 0, BlendMode::Normal),
            [10, 20, 30, 255]
        );
    }

    #[test]
    fn blend_modes_match_basic_opaque_cases() {
        assert_eq!(
            composite_pixel(
                [128, 128, 128, 255],
                [128, 128, 128, 255],
                255,
                BlendMode::Multiply
            ),
            [64, 64, 64, 255]
        );
        assert_eq!(
            composite_pixel(
                [80, 80, 80, 255],
                [160, 160, 160, 255],
                255,
                BlendMode::Difference
            ),
            [80, 80, 80, 255]
        );
        for mode in BlendMode::ALL {
            assert_eq!(BlendMode::from_name(mode.name()), Some(mode));
        }
    }

    #[test]
    fn undo_restores_saved_state_and_discards_redo_after_new_edit() {
        let mut document = Document::new(2, 1).unwrap();
        document.begin_edit("first stroke");
        document.active_pixels_mut()[0] = 0;
        assert!(document.finish_edit());
        document.mark_saved();
        document.begin_edit("second stroke");
        document.active_pixels_mut()[4] = 0;
        document.finish_edit();
        assert!(document.is_dirty());
        assert!(document.undo());
        assert!(!document.is_dirty());
        assert!(document.redo());
        assert!(document.is_dirty());
        document.undo();
        document.begin_edit("replacement stroke");
        document.active_pixels_mut()[5] = 0;
        document.finish_edit();
        assert!(!document.redo());
    }

    #[test]
    fn cancelled_and_empty_edits_do_not_create_history() {
        let mut document = Document::new(2, 2).unwrap();
        document.begin_edit("empty");
        assert!(!document.finish_edit());
        document.begin_edit("cancelled");
        document.active_pixels_mut()[0] = 12;
        assert!(document.is_dirty());
        document.cancel_edit();
        assert!(!document.is_dirty());
        assert_eq!(document.history_labels().len(), 1);
    }

    #[test]
    fn selection_history_does_not_dirty_saved_pixels() {
        let mut document = Document::new(2, 2).unwrap();
        document.begin_edit("selection");
        document.selection = Some(vec![255, 0, 0, 0]);
        document.finish_edit();
        assert!(!document.is_dirty());
        document.undo();
        assert!(document.selection.is_none());
        document.redo();
        assert_eq!(document.selection_bounds(), Some((0, 0, 1, 1)));
    }

    #[test]
    fn layers_merge_and_flatten_preserve_the_composite() {
        let mut document = Document::new(2, 1).unwrap();
        assert!(document.delete_layer().is_err());
        document.add_layer().unwrap();
        document.begin_edit("paint");
        document
            .active_pixels_mut()
            .copy_from_slice(&[255, 0, 0, 128, 0, 0, 255, 255]);
        document.finish_edit();
        let expected = document.composite();
        document.merge_down().unwrap();
        assert_eq!(document.layers.len(), 1);
        assert_eq!(document.composite(), expected);
        document.undo();
        document.flatten().unwrap();
        assert_eq!(document.composite(), expected);
    }

    #[test]
    fn rotate_and_flip_keep_layers_and_selection_aligned() {
        let pixels = vec![
            1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255, 5, 0, 0, 255, 6, 0, 0, 255,
        ];
        let mut document = Document::from_rgba(3, 2, pixels.clone(), "test".into()).unwrap();
        document.selection = Some(vec![255, 0, 0, 0, 0, 0]);
        document.rotate_cw().unwrap();
        assert_eq!((document.width, document.height), (2, 3));
        assert_eq!(
            document
                .active_pixels()
                .chunks_exact(4)
                .map(|p| p[0])
                .collect::<Vec<_>>(),
            [4, 1, 5, 2, 6, 3]
        );
        assert_eq!(document.selection_bounds(), Some((1, 0, 1, 1)));
        document.rotate_ccw().unwrap();
        assert_eq!(document.active_pixels(), pixels);
        document.flip_horizontal().unwrap();
        assert_eq!(document.selection_bounds(), Some((2, 0, 1, 1)));
        document.flip_horizontal().unwrap();
        assert_eq!(document.active_pixels(), pixels);
    }

    #[test]
    fn resizing_uses_premultiplied_alpha_and_centers_canvas() {
        let mut document =
            Document::from_rgba(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0], "test".into()).unwrap();
        document.resize(3, 1).unwrap();
        assert_eq!(&document.active_pixels()[4..8], &[255, 0, 0, 128]);
        document.undo();
        document.canvas_size(4, 3).unwrap();
        assert_eq!(&document.active_pixels()[20..24], &[255, 0, 0, 255]);
        assert_eq!(&document.active_pixels()[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn crop_and_dimension_validation_reject_empty_or_invalid_data() {
        assert!(Document::new(0, 10).is_err());
        assert!(Document::new(u32::MAX, 10).is_err());
        assert!(Document::from_rgba(2, 2, vec![0; 4], "bad".into()).is_err());
        let mut document = Document::new(4, 3).unwrap();
        document.selection = Some(vec![0; 12]);
        assert!(document.crop_selection().is_err());
        document.selection.as_mut().unwrap()[5] = 255;
        document.selection.as_mut().unwrap()[10] = 255;
        document.crop_selection().unwrap();
        assert_eq!((document.width, document.height), (2, 2));
        assert_eq!(document.selection.as_ref().unwrap(), &[255, 0, 0, 255]);
        assert!(document.validate().is_ok());
    }
}
