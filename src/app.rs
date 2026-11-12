use crate::clipboard::{self, ImageData};
use crate::dialogs;
use crate::ui::{Action, Adjustment, Model, Ui};
use macroquad::prelude::*;
use openpaint::document::{Document, Pixel};
use openpaint::tools::{SelectionMode, Tool, ToolSettings};
use openpaint::{effects, io, tools};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

struct Tab {
    document: Document,
    texture: Option<Texture2D>,
    texture_size: (u32, u32),
    dirty_texture: bool,
    zoom: f32,
    pan: Vec2,
}

impl Tab {
    fn new(document: Document) -> Self {
        Self {
            document,
            texture: None,
            texture_size: (0, 0),
            dirty_texture: true,
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

struct Drag {
    tool: Tool,
    start: (f32, f32),
    last: (f32, f32),
    original: Vec<u8>,
    selection: Option<Vec<u8>>,
    points: Vec<(i32, i32)>,
    color: Pixel,
    right: bool,
    clone_offset: (i32, i32),
    copy: bool,
    transform: DragTransform,
}

enum DragTransform {
    Move,
    Rotate {
        center: (f32, f32),
        initial: f32,
    },
    Scale {
        anchor: (f32, f32),
        initial: (f32, f32),
    },
}

struct ClipboardJob {
    paste: Option<(bool, bool)>,
    result: Receiver<Result<Option<ImageData>, String>>,
}
struct FileDialogJob {
    save: bool,
    import_layer: bool,
    tab: usize,
    result: Receiver<Result<Option<PathBuf>, String>>,
}
struct EffectJob {
    tab: usize,
    label: String,
    preview: bool,
    adjustment: Adjustment,
    result: Receiver<Result<Vec<u8>, String>>,
}
struct EffectPreview {
    tab: usize,
    original: Vec<u8>,
}

struct SaveAll {
    pending: VecDeque<usize>,
    original_tab: usize,
}

pub struct Editor {
    tabs: Vec<Tab>,
    active: usize,
    ui: Ui,
    tool: Tool,
    settings: ToolSettings,
    primary: Pixel,
    secondary: Pixel,
    status: String,
    drag: Option<Drag>,
    pan_drag: Option<Vec2>,
    clone_anchor: Option<(i32, i32)>,
    text_position: (i32, i32),
    clipboard: Option<ImageData>,
    clipboard_job: Option<ClipboardJob>,
    file_dialog: Option<FileDialogJob>,
    effect_job: Option<EffectJob>,
    effect_preview: Option<EffectPreview>,
    last_adjustment: Option<Adjustment>,
    save_all: Option<SaveAll>,
    pending_close: Option<usize>,
    closing_all: bool,
    recent: Vec<PathBuf>,
    pub quit: bool,
}

impl Editor {
    pub fn new() -> Self {
        let mut editor = Self::blank();
        editor.read_settings();
        for path in std::env::args_os()
            .skip(1)
            .filter(|p| !p.to_string_lossy().starts_with('-'))
        {
            editor.open_path(PathBuf::from(path));
        }
        editor
    }

    fn blank() -> Self {
        Self {
            tabs: vec![Tab::new(
                Document::new(800, 600).expect("valid default dimensions"),
            )],
            active: 0,
            ui: Ui::new(),
            tool: Tool::Paintbrush,
            settings: ToolSettings::default(),
            primary: [0, 0, 0, 255],
            secondary: [255, 255, 255, 255],
            status: "ready".into(),
            drag: None,
            pan_drag: None,
            clone_anchor: None,
            text_position: (20, 20),
            clipboard: None,
            file_dialog: None,
            effect_job: None,
            effect_preview: None,
            last_adjustment: None,
            save_all: None,
            clipboard_job: None,
            pending_close: None,
            closing_all: false,
            recent: Vec::new(),
            quit: false,
        }
    }

    fn busy(&self) -> bool {
        self.file_dialog.is_some() || self.effect_job.is_some() || self.clipboard_job.is_some()
    }

    pub fn frame(&mut self) {
        self.poll_jobs();
        self.continue_save_all();
        if self.quit {
            return;
        }
        clear_background(crate::ui::WORKSPACE);
        let viewport = self.ui.viewport();
        self.draw_canvas(viewport);
        let names: Vec<_> = self
            .tabs
            .iter()
            .map(|t| (t.document.name.clone(), t.document.is_dirty()))
            .collect();
        let thumbnails: Vec<_> = self.tabs.iter().map(|tab| tab.texture.clone()).collect();
        let output = self.ui.draw(Model {
            doc: &self.tabs[self.active].document,
            tabs: &names,
            thumbnails: &thumbnails,
            active: self.active,
            tool: self.tool,
            settings: &self.settings,
            primary: self.primary,
            secondary: self.secondary,
            zoom: self.tabs[self.active].zoom,
            status: &self.status,
            busy: self.busy(),
            can_repeat: self.last_adjustment.is_some(),
        });
        for action in output.actions {
            if self.busy() && !matches!(action, Action::CancelEffect) {
                continue;
            }
            if self.save_all.is_some()
                && !matches!(action, Action::SavePath(_) | Action::ConfirmCancel)
            {
                continue;
            }
            self.finish_drag();
            self.action(action);
            if self.quit {
                return;
            }
        }
        if !self.busy() && self.save_all.is_none() {
            if !output.keyboard_captured {
                self.shortcuts();
            }
            if !output.pointer_captured && !self.ui.has_modal() {
                self.canvas_input(output.viewport);
            } else if self.drag.is_some()
                && !is_mouse_button_down(MouseButton::Left)
                && !is_mouse_button_down(MouseButton::Right)
            {
                self.finish_drag();
            }
        }
        if is_quit_requested() && !self.busy() {
            self.cancel_save_all();
            self.finish_drag();
            self.cancel_effect();
            self.closing_all = true;
            self.request_close(self.active);
        }
        for file in get_dropped_files() {
            if !self.busy() && self.save_all.is_none() && !self.ui.has_modal() {
                if let Some(path) = file.path {
                    self.open_path(path);
                }
            }
        }
    }

    fn draw_canvas(&mut self, viewport: Rect) {
        let busy = self.busy();
        let tab = &mut self.tabs[self.active];
        if tab.dirty_texture || tab.texture.is_none() {
            let pixels = tab.document.composite();
            let size = (tab.document.width, tab.document.height);
            let image = Image {
                bytes: pixels,
                width: size.0 as u16,
                height: size.1 as u16,
            };
            if tab.texture_size == size {
                if let Some(texture) = &tab.texture {
                    texture.update(&image);
                }
            } else {
                let texture = Texture2D::from_image(&image);
                texture.set_filter(FilterMode::Nearest);
                tab.texture = Some(texture);
                tab.texture_size = size;
            }
            tab.dirty_texture = false;
        }
        let rect = Self::image_rect(tab, viewport);
        let visible = intersection(rect, viewport);
        if let Some(visible) = visible {
            draw_rectangle(
                visible.x,
                visible.y,
                visible.w,
                visible.h,
                crate::ui::CHECKER_LIGHT,
            );
            let tile = 12.0;
            let start_x = (visible.x / tile).floor() as i32;
            let start_y = (visible.y / tile).floor() as i32;
            for y in start_y..=(visible.bottom() / tile).ceil() as i32 {
                for x in start_x..=(visible.right() / tile).ceil() as i32 {
                    if (x + y) % 2 == 0 {
                        if let Some(cell) = intersection(
                            visible,
                            Rect::new(x as f32 * tile, y as f32 * tile, tile, tile),
                        ) {
                            draw_rectangle(cell.x, cell.y, cell.w, cell.h, crate::ui::CHECKER_DARK);
                        }
                    }
                }
            }
            if let Some(texture) = &tab.texture {
                draw_texture_ex(
                    texture,
                    visible.x,
                    visible.y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(visible.w, visible.h)),
                        source: Some(Rect::new(
                            (visible.x - rect.x) / tab.zoom,
                            (visible.y - rect.y) / tab.zoom,
                            visible.w / tab.zoom,
                            visible.h / tab.zoom,
                        )),
                        ..Default::default()
                    },
                );
            }
            draw_rectangle_lines(
                visible.x,
                visible.y,
                visible.w,
                visible.h,
                1.0,
                crate::ui::BORDER,
            );
            if let Some(mask) = &tab.document.selection {
                let min_x = ((visible.x - rect.x) / tab.zoom).max(0.0) as u32;
                let min_y = ((visible.y - rect.y) / tab.zoom).max(0.0) as u32;
                let max_x =
                    (((visible.right() - rect.x) / tab.zoom).ceil() as u32).min(tab.document.width);
                let max_y = (((visible.bottom() - rect.y) / tab.zoom).ceil() as u32)
                    .min(tab.document.height);
                let stride = if tab.zoom < 1.0 {
                    (1.0 / tab.zoom).floor() as usize
                } else {
                    1
                };
                for y in (min_y..max_y).step_by(stride.max(1)) {
                    for x in (min_x..max_x).step_by(stride.max(1)) {
                        let index = (y * tab.document.width + x) as usize;
                        if mask[index] == 0 {
                            continue;
                        }
                        let edge = x == 0
                            || y == 0
                            || x + 1 == tab.document.width
                            || y + 1 == tab.document.height
                            || mask[index - 1] == 0
                            || mask[index + 1] == 0
                            || mask[index - tab.document.width as usize] == 0
                            || mask[index + tab.document.width as usize] == 0;
                        if edge {
                            let color = if (x + y) % 8 < 4 { BLACK } else { WHITE };
                            draw_rectangle(
                                rect.x + x as f32 * tab.zoom,
                                rect.y + y as f32 * tab.zoom,
                                tab.zoom.max(1.0),
                                tab.zoom.max(1.0),
                                color,
                            );
                        }
                    }
                }
            }
            if matches!(self.tool, Tool::MovePixels | Tool::MoveSelection) {
                let (x, y, w, h) = tab.document.selection_bounds().unwrap_or((
                    0,
                    0,
                    tab.document.width,
                    tab.document.height,
                ));
                for (x, y) in [(x, y), (x + w, y), (x, y + h), (x + w, y + h)] {
                    let point = vec2(rect.x + x as f32 * tab.zoom, rect.y + y as f32 * tab.zoom);
                    if viewport.contains(point) {
                        draw_rectangle(point.x - 3.0, point.y - 3.0, 7.0, 7.0, WHITE);
                        draw_rectangle_lines(point.x - 3.0, point.y - 3.0, 7.0, 7.0, 1.0, BLACK);
                    }
                }
            }
        }
        let mouse = Vec2::from(mouse_position());
        if viewport.contains(mouse)
            && !self.ui.has_modal()
            && !busy
            && matches!(
                self.tool,
                Tool::Paintbrush | Tool::Eraser | Tool::CloneStamp | Tool::Recolor
            )
        {
            let radius = (self.settings.size * tab.zoom / 2.0).max(2.0);
            if mouse.x - radius >= viewport.x
                && mouse.x + radius <= viewport.right()
                && mouse.y - radius >= viewport.y
                && mouse.y + radius <= viewport.bottom()
            {
                draw_circle_lines(mouse.x, mouse.y, radius, 1.0, BLACK);
                draw_circle_lines(mouse.x, mouse.y, radius + 1.0, 1.0, WHITE);
            }
        }
    }

    fn image_rect(tab: &Tab, viewport: Rect) -> Rect {
        let w = tab.document.width as f32 * tab.zoom;
        let h = tab.document.height as f32 * tab.zoom;
        Rect::new(
            viewport.x + (viewport.w - w) / 2.0 + tab.pan.x,
            viewport.y + (viewport.h - h) / 2.0 + tab.pan.y,
            w,
            h,
        )
    }

    fn canvas_input(&mut self, viewport: Rect) {
        let mouse = Vec2::from(mouse_position());
        let inside = viewport.contains(mouse);
        let (_, wheel) = mouse_wheel();
        if inside && wheel != 0.0 {
            let tab = &mut self.tabs[self.active];
            let old = tab.zoom;
            tab.zoom = (tab.zoom * 1.15f32.powf(wheel)).clamp(0.05, 32.0);
            let center = vec2(viewport.x + viewport.w / 2.0, viewport.y + viewport.h / 2.0);
            tab.pan = mouse - center - (mouse - center - tab.pan) * (tab.zoom / old);
        }
        let panning = is_mouse_button_down(MouseButton::Middle)
            || (is_key_down(KeyCode::Space) && is_mouse_button_down(MouseButton::Left))
            || (self.tool == Tool::Pan && is_mouse_button_down(MouseButton::Left));
        if panning && (inside || self.pan_drag.is_some()) {
            if let Some(last) = self.pan_drag {
                self.tabs[self.active].pan += mouse - last;
            }
            self.pan_drag = Some(mouse);
            return;
        }
        self.pan_drag = None;
        let tab = &self.tabs[self.active];
        let rect = Self::image_rect(tab, viewport);
        let point = ((mouse.x - rect.x) / tab.zoom, (mouse.y - rect.y) / tab.zoom);
        let in_image = inside
            && (rect.contains(mouse)
                || (matches!(self.tool, Tool::MovePixels | Tool::MoveSelection)
                    && Rect::new(rect.x - 8.0, rect.y - 8.0, rect.w + 16.0, rect.h + 16.0)
                        .contains(mouse)));
        let pressed = is_mouse_button_pressed(MouseButton::Left)
            || is_mouse_button_pressed(MouseButton::Right);
        let right = is_mouse_button_pressed(MouseButton::Right);
        if pressed && in_image {
            self.start_drag(point, right);
        }
        if self.drag.is_some() {
            let held = self.drag.as_ref().is_some_and(|d| {
                is_mouse_button_down(if d.right {
                    MouseButton::Right
                } else {
                    MouseButton::Left
                })
            });
            if held && self.drag.as_ref().is_some_and(|drag| drag.last != point) {
                self.update_drag(point);
            } else if !held {
                self.finish_drag();
            }
        }
    }

    fn start_drag(&mut self, point: (f32, f32), right: bool) {
        let color = if right { self.secondary } else { self.primary };
        let x = point.0.floor() as i32;
        let y = point.1.floor() as i32;
        if self.tool == Tool::Zoom {
            self.tabs[self.active].zoom =
                (self.tabs[self.active].zoom * if right { 0.5 } else { 2.0 }).clamp(0.05, 32.0);
            return;
        }
        if self.tool == Tool::Pan {
            return;
        }
        if self.tool == Tool::Text {
            self.text_position = (x, y);
            self.ui.show_text();
            return;
        }
        let zoom = self.tabs[self.active].zoom;
        let doc = &mut self.tabs[self.active].document;
        if self.tool == Tool::ColorPicker {
            let index = ((y as u32 * doc.width + x as u32) * 4) as usize;
            let pixels = if ctrl() {
                doc.composite()
            } else {
                doc.active_pixels().to_vec()
            };
            let sampled: Pixel = pixels[index..index + 4].try_into().unwrap();
            if right {
                self.secondary = sampled;
            } else {
                self.primary = sampled;
            }
            return;
        }
        if self.tool == Tool::CloneStamp && ctrl() {
            self.clone_anchor = Some((x, y));
            self.status = "clone source set".into();
            return;
        }
        if self.tool == Tool::CloneStamp && self.clone_anchor.is_none() {
            self.status = "ctrl-click to set the clone source".into();
            return;
        }
        doc.begin_edit(self.tool.name());
        let global = shift() || !self.settings.contiguous;
        let mode = selection_mode(self.settings.selection_mode, right);
        if self.tool == Tool::PaintBucket {
            tools::flood_fill(doc, x, y, color, &self.settings, global);
            doc.finish_edit();
            self.tabs[self.active].dirty_texture = true;
            return;
        }
        if self.tool == Tool::MagicWand {
            tools::magic_wand(doc, x, y, &self.settings, mode, global);
            doc.finish_edit();
            return;
        }
        let mut drag = Drag {
            tool: self.tool,
            start: point,
            last: point,
            original: doc.active_pixels().to_vec(),
            selection: doc.selection.clone(),
            points: vec![(x, y)],
            color,
            right,
            clone_offset: (0, 0),
            copy: ctrl(),
            transform: DragTransform::Move,
        };
        if matches!(self.tool, Tool::MovePixels | Tool::MoveSelection) {
            let (x, y, w, h) = doc
                .selection_bounds()
                .unwrap_or((0, 0, doc.width, doc.height));
            let center = (x as f32 + w as f32 / 2.0, y as f32 + h as f32 / 2.0);
            if right {
                drag.transform = DragTransform::Rotate {
                    center,
                    initial: (point.1 - center.1).atan2(point.0 - center.0),
                };
            } else {
                let corners = [
                    (x as f32, y as f32),
                    ((x + w) as f32, y as f32),
                    (x as f32, (y + h) as f32),
                    ((x + w) as f32, (y + h) as f32),
                ];
                for (index, corner) in corners.iter().enumerate() {
                    if (point.0 - corner.0).hypot(point.1 - corner.1) * zoom <= 9.0 {
                        let anchor = corners[3 - index];
                        drag.transform = DragTransform::Scale {
                            anchor,
                            initial: (point.0 - anchor.0, point.1 - anchor.1),
                        };
                        break;
                    }
                }
            }
        }
        if let Some(anchor) = self.clone_anchor {
            drag.clone_offset = (anchor.0 - x, anchor.1 - y);
        }
        self.drag = Some(drag);
        self.update_drag(point);
    }

    fn update_drag(&mut self, point: (f32, f32)) {
        let Some(drag) = &mut self.drag else { return };
        let tab = &mut self.tabs[self.active];
        let doc = &mut tab.document;
        let mut end = point;
        if shift()
            && matches!(
                drag.tool,
                Tool::RectangleSelect
                    | Tool::EllipseSelect
                    | Tool::Rectangle
                    | Tool::Ellipse
                    | Tool::RoundedRectangle
            )
        {
            let dx = end.0 - drag.start.0;
            let dy = end.1 - drag.start.1;
            let extent = dx.abs().max(dy.abs());
            end = (
                drag.start.0 + extent * dx.signum(),
                drag.start.1 + extent * dy.signum(),
            );
        }
        if shift() && matches!(drag.tool, Tool::Line | Tool::Gradient) {
            let dx = end.0 - drag.start.0;
            let dy = end.1 - drag.start.1;
            let angle =
                (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
            let distance = dx.hypot(dy);
            end = (
                drag.start.0 + angle.cos() * distance,
                drag.start.1 + angle.sin() * distance,
            );
        }
        let mode = selection_mode(self.settings.selection_mode, drag.right);
        match drag.tool {
            Tool::Paintbrush | Tool::Pencil | Tool::Eraser => {
                let mut settings = self.settings.clone();
                if drag.tool == Tool::Pencil {
                    settings.size = 1.0;
                    settings.hardness = 1.0;
                }
                tools::stroke(
                    doc,
                    drag.last,
                    end,
                    drag.color,
                    &settings,
                    drag.tool == Tool::Eraser,
                );
            }
            Tool::CloneStamp => tools::clone_stroke(
                doc,
                &drag.original,
                drag.last,
                end,
                drag.clone_offset,
                &self.settings,
            ),
            Tool::Recolor => tools::recolor_stroke(
                doc,
                drag.last,
                end,
                if drag.right {
                    self.secondary
                } else {
                    self.primary
                },
                if drag.right {
                    self.primary
                } else {
                    self.secondary
                },
                &self.settings,
            ),
            Tool::RectangleSelect | Tool::EllipseSelect => {
                doc.selection = drag.selection.clone();
                let start = (drag.start.0 as i32, drag.start.1 as i32);
                let end = tools::selection_end(start, (end.0 as i32, end.1 as i32), &self.settings);
                tools::select_rect(doc, start, end, drag.tool == Tool::EllipseSelect, mode);
            }
            Tool::Lasso => {
                let next = (end.0 as i32, end.1 as i32);
                if drag.points.last() != Some(&next) {
                    drag.points.push(next);
                }
                doc.selection = drag.selection.clone();
                tools::select_lasso(doc, &drag.points, mode);
            }
            Tool::Line | Tool::Rectangle | Tool::Ellipse | Tool::RoundedRectangle => {
                doc.active_pixels_mut().copy_from_slice(&drag.original);
                let start = if alt() {
                    (2.0 * drag.start.0 - end.0, 2.0 * drag.start.1 - end.1)
                } else {
                    drag.start
                };
                tools::draw_shape(
                    doc,
                    start,
                    end,
                    drag.color,
                    if drag.right {
                        self.primary
                    } else {
                        self.secondary
                    },
                    &self.settings,
                    drag.tool,
                );
            }
            Tool::Gradient => {
                doc.active_pixels_mut().copy_from_slice(&drag.original);
                tools::gradient(
                    doc,
                    drag.start,
                    end,
                    drag.color,
                    if drag.right {
                        self.primary
                    } else {
                        self.secondary
                    },
                    &self.settings,
                );
            }
            Tool::MovePixels => {
                if let Some(transform) = drag_transform(&drag.transform, end, shift()) {
                    tools::transform_pixels(
                        doc,
                        &drag.original,
                        drag.selection.as_deref(),
                        &transform,
                        drag.copy,
                    );
                } else {
                    tools::move_pixels(
                        doc,
                        &drag.original,
                        drag.selection.as_deref(),
                        (end.0 - drag.start.0).round() as i32,
                        (end.1 - drag.start.1).round() as i32,
                        drag.copy,
                    );
                }
            }
            Tool::MoveSelection => {
                if let Some(transform) = drag_transform(&drag.transform, end, shift()) {
                    tools::transform_selection(doc, drag.selection.as_deref(), &transform);
                } else {
                    tools::move_selection(
                        doc,
                        drag.selection.as_deref(),
                        (end.0 - drag.start.0).round() as i32,
                        (end.1 - drag.start.1).round() as i32,
                    );
                }
            }
            _ => {}
        }
        drag.last = end;
        tab.dirty_texture = true;
    }

    fn finish_drag(&mut self) {
        if self.drag.take().is_some() {
            self.tabs[self.active].document.finish_edit();
            self.tabs[self.active].dirty_texture = true;
        }
    }

    fn shortcuts(&mut self) {
        if is_key_pressed(KeyCode::Escape) {
            if self.drag.take().is_some() {
                self.tabs[self.active].document.cancel_edit();
                self.tabs[self.active].dirty_texture = true;
            } else {
                self.action(Action::Deselect);
            }
            return;
        }
        if ctrl() {
            let action = if is_key_pressed(KeyCode::N) {
                self.ui.show_new(800, 600);
                None
            } else if is_key_pressed(KeyCode::O) {
                Some(Action::Open)
            } else if is_key_pressed(KeyCode::S) {
                Some(if alt() {
                    Action::SaveAll
                } else if shift() {
                    Action::SaveAs
                } else {
                    Action::Save
                })
            } else if is_key_pressed(KeyCode::W) {
                Some(Action::CloseTab(self.active))
            } else if is_key_pressed(KeyCode::Z) {
                Some(if shift() { Action::Redo } else { Action::Undo })
            } else if is_key_pressed(KeyCode::Y) {
                Some(Action::Redo)
            } else if is_key_pressed(KeyCode::A) {
                Some(Action::SelectAll)
            } else if is_key_pressed(KeyCode::D) {
                Some(if shift() {
                    Action::DuplicateLayer
                } else {
                    Action::Deselect
                })
            } else if is_key_pressed(KeyCode::I) {
                Some(Action::InvertSelection)
            } else if is_key_pressed(KeyCode::C) {
                Some(if shift() {
                    Action::CopyMerged
                } else {
                    Action::Copy
                })
            } else if is_key_pressed(KeyCode::X) {
                Some(Action::Cut)
            } else if is_key_pressed(KeyCode::V) {
                Some(if alt() {
                    Action::PasteNewImage
                } else if shift() {
                    Action::PasteNewLayer
                } else {
                    Action::Paste
                })
            } else if is_key_pressed(KeyCode::R) {
                let doc = &self.tabs[self.active].document;
                self.ui
                    .show_resize(doc.width, doc.height, shift(), self.primary, self.secondary);
                None
            } else if is_key_pressed(KeyCode::B) {
                Some(if shift() {
                    Action::FitSelection
                } else {
                    Action::Fit
                })
            } else if is_key_pressed(KeyCode::F) {
                Some(if shift() {
                    Action::Flatten
                } else {
                    Action::RepeatEffect
                })
            } else if is_key_pressed(KeyCode::Key0) {
                Some(Action::ActualSize)
            } else if is_key_pressed(KeyCode::Tab) {
                Some(Action::SwitchTab((self.active + 1) % self.tabs.len()))
            } else if shift() && is_key_pressed(KeyCode::Insert) {
                Some(Action::AddLayer)
            } else if shift() && is_key_pressed(KeyCode::Delete) {
                Some(Action::DeleteLayer)
            } else if is_key_pressed(KeyCode::M) {
                Some(Action::MergeDown)
            } else {
                None
            };
            if let Some(action) = action {
                self.finish_drag();
                self.action(action);
            }
            return;
        }
        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.action(Action::DeleteSelection);
        }
        if is_key_pressed(KeyCode::X) {
            self.action(Action::SwapColors);
        }
        if is_key_pressed(KeyCode::C) {
            self.action(Action::ResetColors);
        }
        if is_key_pressed(KeyCode::F4) {
            self.ui
                .show_layer_properties(&self.tabs[self.active].document);
        }
        if is_key_pressed(KeyCode::LeftBracket) {
            self.settings.size = (self.settings.size - 1.0).max(1.0);
        }
        if is_key_pressed(KeyCode::RightBracket) {
            self.settings.size = (self.settings.size + 1.0).min(512.0);
        }
        let tool = if is_key_pressed(KeyCode::B) {
            Some(Tool::Paintbrush)
        } else if is_key_pressed(KeyCode::P) {
            Some(Tool::Pencil)
        } else if is_key_pressed(KeyCode::E) {
            Some(Tool::Eraser)
        } else if is_key_pressed(KeyCode::F) {
            Some(Tool::PaintBucket)
        } else if is_key_pressed(KeyCode::G) {
            Some(Tool::Gradient)
        } else if is_key_pressed(KeyCode::K) {
            Some(Tool::ColorPicker)
        } else if is_key_pressed(KeyCode::L) {
            Some(Tool::CloneStamp)
        } else if is_key_pressed(KeyCode::R) {
            Some(Tool::Recolor)
        } else if is_key_pressed(KeyCode::T) {
            Some(Tool::Text)
        } else if is_key_pressed(KeyCode::H) {
            Some(Tool::Pan)
        } else if is_key_pressed(KeyCode::Z) {
            Some(Tool::Zoom)
        } else if is_key_pressed(KeyCode::M) {
            Some(if self.tool == Tool::MovePixels {
                Tool::MoveSelection
            } else {
                Tool::MovePixels
            })
        } else if is_key_pressed(KeyCode::O) {
            Some(match self.tool {
                Tool::Line => Tool::Rectangle,
                Tool::Rectangle => Tool::Ellipse,
                Tool::Ellipse => Tool::RoundedRectangle,
                _ => Tool::Line,
            })
        } else if is_key_pressed(KeyCode::S) {
            Some(match self.tool {
                Tool::RectangleSelect => Tool::Lasso,
                Tool::Lasso => Tool::EllipseSelect,
                Tool::EllipseSelect => Tool::MagicWand,
                _ => Tool::RectangleSelect,
            })
        } else {
            None
        };
        if let Some(tool) = tool {
            self.finish_drag();
            self.tool = tool;
            self.status = tool.name().into();
        }
        let delta = if is_key_pressed(KeyCode::Left) {
            Some((-1, 0))
        } else if is_key_pressed(KeyCode::Right) {
            Some((1, 0))
        } else if is_key_pressed(KeyCode::Up) {
            Some((0, -1))
        } else if is_key_pressed(KeyCode::Down) {
            Some((0, 1))
        } else {
            None
        };
        if let Some((x, y)) = delta {
            let doc = &mut self.tabs[self.active].document;
            let pixels = doc.active_pixels().to_vec();
            let mask = doc.selection.clone();
            let n = if shift() { 10 } else { 1 };
            if matches!(self.tool, Tool::MovePixels | Tool::MoveSelection) {
                doc.begin_edit("Nudge selection");
                if self.tool == Tool::MovePixels {
                    tools::move_pixels(doc, &pixels, mask.as_deref(), x * n, y * n, false);
                } else {
                    tools::move_selection(doc, mask.as_deref(), x * n, y * n);
                }
                doc.finish_edit();
                self.tabs[self.active].dirty_texture = true;
            }
        }
    }

    fn action(&mut self, action: Action) {
        let mut result: Result<(), String> = Ok(());
        match action {
            Action::New { width, height } => match Document::new(width, height) {
                Ok(doc) => {
                    self.tabs.push(Tab::new(doc));
                    self.active = self.tabs.len() - 1;
                }
                Err(error) => result = Err(error),
            },
            Action::Open => self.choose_file(false),
            Action::OpenRecent(index) => {
                if let Some(path) = self.recent.get(index).cloned() {
                    self.open_path(path);
                }
            }
            Action::OpenPath(path) => self.open_path(PathBuf::from(path)),
            Action::Save => self.save_current(false),
            Action::SaveAs => self.save_current(true),
            Action::SaveAll => {
                self.save_all = Some(SaveAll {
                    pending: self
                        .tabs
                        .iter()
                        .enumerate()
                        .filter_map(|(index, tab)| tab.document.is_dirty().then_some(index))
                        .collect(),
                    original_tab: self.active,
                });
            }
            Action::SavePath(path) => self.save_path(self.active, PathBuf::from(path)),
            Action::CloseTab(index) => self.request_close(index),
            Action::SwitchTab(index) => {
                if index < self.tabs.len() {
                    self.active = index;
                }
            }
            Action::Quit => {
                self.closing_all = true;
                self.request_close(self.active);
            }
            Action::Undo => {
                self.tabs[self.active].document.undo();
            }
            Action::Redo => {
                self.tabs[self.active].document.redo();
            }
            Action::SelectAll => {
                let doc = &mut self.tabs[self.active].document;
                doc.begin_edit("Select all");
                doc.selection = Some(vec![255; (doc.width * doc.height) as usize]);
                doc.finish_edit();
            }
            Action::Deselect => {
                let doc = &mut self.tabs[self.active].document;
                doc.begin_edit("Deselect");
                doc.selection = None;
                doc.finish_edit();
            }
            Action::InvertSelection => {
                let doc = &mut self.tabs[self.active].document;
                doc.begin_edit("Invert selection");
                tools::invert_selection(doc);
                doc.finish_edit();
            }
            Action::DeleteSelection => {
                let doc = &mut self.tabs[self.active].document;
                doc.begin_edit("Erase selection");
                tools::delete_selection(doc);
                doc.finish_edit();
            }
            Action::Copy => self.copy(false, false),
            Action::CopyMerged => self.copy(false, true),
            Action::Cut => self.copy(true, false),
            Action::Paste => self.read_clipboard(false, false),
            Action::PasteNewLayer => self.read_clipboard(true, false),
            Action::PasteNewImage => self.read_clipboard(false, true),
            Action::Tool(tool) => {
                self.tool = tool;
                self.status = tool.name().into();
            }
            Action::Settings(settings) => self.settings = settings,
            Action::Primary(color) => self.primary = color,
            Action::Secondary(color) => self.secondary = color,
            Action::SwapColors => std::mem::swap(&mut self.primary, &mut self.secondary),
            Action::ResetColors => {
                self.primary = [0, 0, 0, 255];
                self.secondary = [255, 255, 255, 255];
            }
            Action::Zoom(zoom) => self.tabs[self.active].zoom = zoom.clamp(0.05, 32.0),
            Action::ActualSize => {
                self.tabs[self.active].zoom = 1.0;
                self.tabs[self.active].pan = Vec2::ZERO;
            }
            Action::Fit => {
                let view = self.ui.viewport();
                let tab = &mut self.tabs[self.active];
                tab.zoom = ((view.w - 24.0) / tab.document.width as f32)
                    .min((view.h - 24.0) / tab.document.height as f32)
                    .clamp(0.05, 32.0);
                tab.pan = Vec2::ZERO;
            }
            Action::FitSelection => {
                let view = self.ui.viewport();
                let tab = &mut self.tabs[self.active];
                if let Some((x, y, width, height)) = tab.document.selection_bounds() {
                    tab.zoom = ((view.w - 24.0) / width as f32)
                        .min((view.h - 24.0) / height as f32)
                        .clamp(0.05, 32.0);
                    tab.pan = vec2(
                        tab.document.width as f32 / 2.0 - x as f32 - width as f32 / 2.0,
                        tab.document.height as f32 / 2.0 - y as f32 - height as f32 / 2.0,
                    ) * tab.zoom;
                }
            }
            Action::AddLayer => result = self.tabs[self.active].document.add_layer(),
            Action::ImportLayer => {
                self.choose_file(false);
                if let Some(job) = &mut self.file_dialog {
                    job.import_layer = true;
                }
            }
            Action::ImportLayerPath(path) => self.import_path(self.active, PathBuf::from(path)),
            Action::DuplicateLayer => result = self.tabs[self.active].document.duplicate_layer(),
            Action::DeleteLayer => result = self.tabs[self.active].document.delete_layer(),
            Action::MergeDown => result = self.tabs[self.active].document.merge_down(),
            Action::Flatten => result = self.tabs[self.active].document.flatten(),
            Action::MoveLayer(delta) => result = self.tabs[self.active].document.move_layer(delta),
            Action::SelectLayer(index) => {
                let doc = &mut self.tabs[self.active].document;
                if index < doc.layers.len() {
                    doc.active_layer = index;
                }
            }
            Action::ToggleLayer(index) => {
                let doc = &mut self.tabs[self.active].document;
                if index < doc.layers.len() {
                    doc.begin_edit("Layer visibility");
                    doc.layers[index].visible = !doc.layers[index].visible;
                    doc.finish_edit();
                }
            }
            Action::LayerProperties {
                name,
                opacity,
                blend,
            } => {
                let doc = &mut self.tabs[self.active].document;
                doc.begin_edit("Layer properties");
                let layer = &mut doc.layers[doc.active_layer];
                layer.name = name;
                layer.opacity = opacity;
                layer.blend_mode = blend;
                doc.finish_edit();
            }
            Action::Resize {
                width,
                height,
                canvas,
                anchor,
                fill,
            } => {
                result = if canvas {
                    self.tabs[self.active]
                        .document
                        .canvas_size_with_options(width, height, anchor, fill)
                } else {
                    self.tabs[self.active].document.resize(width, height)
                };
            }
            Action::Crop => result = self.tabs[self.active].document.crop_selection(),
            Action::FlipHorizontal => result = self.tabs[self.active].document.flip_horizontal(),
            Action::FlipVertical => result = self.tabs[self.active].document.flip_vertical(),
            Action::RotateCw => result = self.tabs[self.active].document.rotate_cw(),
            Action::RotateCcw => result = self.tabs[self.active].document.rotate_ccw(),
            Action::Text { text, size } => {
                let doc = &mut self.tabs[self.active].document;
                doc.begin_edit("Text");
                result = tools::text(doc, self.text_position, &text, size, self.primary);
                if result.is_ok() {
                    doc.finish_edit();
                } else {
                    doc.cancel_edit();
                }
            }
            Action::Effect {
                name,
                value,
                second,
            } => self.start_effect(name, value, second, false),
            Action::PreviewEffect {
                name,
                value,
                second,
            } => self.start_effect(name, value, second, true),
            Action::CancelEffect => self.cancel_effect(),
            Action::Adjust {
                adjustment,
                preview,
            } => self.start_adjustment(adjustment, preview),
            Action::RepeatEffect => {
                if let Some(adjustment) = &self.last_adjustment {
                    self.start_adjustment(adjustment.clone(), false);
                }
            }
            Action::ConfirmSave => {
                if let Some(index) = self.pending_close {
                    self.active = index;
                    self.save_current(false);
                }
            }
            Action::ConfirmDiscard => {
                if let Some(index) = self.pending_close.take() {
                    self.close_tab(index);
                }
            }
            Action::ConfirmCancel => {
                self.cancel_save_all();
                self.pending_close = None;
                self.closing_all = false;
            }
        }
        if let Err(error) = result {
            self.error(error);
        }
        if !self.quit {
            self.tabs[self.active].dirty_texture = true;
        }
    }

    fn add_document(&mut self, doc: Document) {
        let untouched = self.tabs.len() == 1
            && !self.tabs[0].document.is_dirty()
            && self.tabs[0].document.path.is_none()
            && self.tabs[0].document.history_labels().len() == 1;
        if untouched {
            self.tabs[0] = Tab::new(doc);
            self.active = 0;
        } else {
            self.tabs.push(Tab::new(doc));
            self.active = self.tabs.len() - 1;
        }
        self.status = "ready".into();
    }

    fn open_path(&mut self, path: PathBuf) {
        match io::open(&path) {
            Ok(doc) => {
                self.add_document(doc);
                self.remember(path);
            }
            Err(error) => self.error(error),
        }
    }

    fn choose_file(&mut self, save: bool) {
        let initial = if save {
            self.tabs[self.active]
                .document
                .path
                .clone()
                .filter(|p| {
                    p.extension().is_some_and(|e| e == "opaint")
                        || self.tabs[self.active].document.layers.len() == 1
                })
                .unwrap_or_else(|| PathBuf::from("Untitled.opaint"))
        } else {
            self.recent
                .first()
                .and_then(|p| p.parent())
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf()
        };
        let (sender, result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(dialogs::choose_file(save, &initial));
        });
        self.file_dialog = Some(FileDialogJob {
            save,
            import_layer: false,
            tab: self.active,
            result,
        });
        self.status = "waiting for file dialog".into();
    }

    fn import_path(&mut self, tab: usize, path: PathBuf) {
        let result =
            io::open(&path).and_then(|source| self.tabs[tab].document.import_layers(&source));
        match result {
            Ok(()) => {
                self.tabs[tab].dirty_texture = true;
                self.status = "imported layers".into();
            }
            Err(error) => self.error(error),
        }
    }

    fn save_current(&mut self, choose: bool) {
        let doc = &self.tabs[self.active].document;
        if !choose {
            if let Some(path) = &doc.path {
                if doc.layers.len() == 1 || path.extension().is_some_and(|e| e == "opaint") {
                    self.save_path(self.active, path.clone());
                    return;
                }
            }
        }
        self.choose_file(true);
    }

    fn save_path(&mut self, index: usize, mut path: PathBuf) {
        if path.extension().is_none() {
            path.set_extension("opaint");
        }
        match io::save(&mut self.tabs[index].document, &path) {
            Ok(saved) => {
                self.remember(path);
                self.status = if saved {
                    "saved".into()
                } else {
                    "exported image; save .opaint to preserve the complete document".into()
                };
                if !saved {
                    self.cancel_save_all();
                }
                if saved && self.pending_close == Some(index) {
                    self.pending_close = None;
                    self.close_tab(index);
                } else if self.pending_close.is_some() {
                    self.pending_close = None;
                    self.closing_all = false;
                    self.ui.show_message("the image was exported. save an .opaint project to preserve the complete document before closing.");
                }
            }
            Err(error) => {
                self.cancel_save_all();
                self.pending_close = None;
                self.closing_all = false;
                self.error(error);
            }
        }
    }

    fn continue_save_all(&mut self) {
        if self.save_all.is_some() && !self.busy() && !self.ui.has_modal() {
            let next = self
                .save_all
                .as_mut()
                .and_then(|state| state.pending.pop_front());
            if let Some(index) = next {
                if index < self.tabs.len() {
                    self.active = index;
                    self.save_current(false);
                }
            } else {
                self.cancel_save_all();
                self.status = "all modified documents saved".into();
            }
        }
    }

    fn cancel_save_all(&mut self) {
        if let Some(state) = self.save_all.take() {
            if !self.tabs.is_empty() {
                self.active = state.original_tab.min(self.tabs.len() - 1);
            }
        }
    }

    fn request_close(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs[index].document.is_dirty() {
            self.pending_close = Some(index);
            self.active = index;
            self.ui.show_confirm(&format!(
                "save changes to {}?",
                self.tabs[index].document.name
            ));
        } else {
            self.close_tab(index);
        }
    }

    fn close_tab(&mut self, index: usize) {
        self.tabs.remove(index);
        if self.closing_all && self.tabs.is_empty() {
            self.write_settings();
            self.quit = true;
            return;
        }
        if self.tabs.is_empty() {
            self.tabs.push(Tab::new(Document::new(800, 600).unwrap()));
        }
        self.active = self.active.min(self.tabs.len() - 1);
        if self.closing_all {
            self.request_close(self.active);
        }
    }

    fn copy(&mut self, cut: bool, merged: bool) {
        let doc = &mut self.tabs[self.active].document;
        let (width, height, pixels) = if merged {
            tools::selected_pixels_from(doc, &doc.composite())
        } else {
            tools::selected_pixels(doc)
        };
        if width == 0 || height == 0 {
            return;
        }
        self.clipboard = Some(ImageData {
            width,
            height,
            pixels,
        });
        if cut {
            doc.begin_edit("Cut");
            tools::delete_selection(doc);
            doc.finish_edit();
        }
        let data = self.clipboard.as_ref().unwrap().clone();
        let (sender, result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(clipboard::write(&data).map(|_| None));
        });
        self.clipboard_job = Some(ClipboardJob {
            paste: None,
            result,
        });
        self.status = "copying pixels".into();
    }

    fn read_clipboard(&mut self, layer: bool, image: bool) {
        let (sender, result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(clipboard::read().map(Some));
        });
        self.clipboard_job = Some(ClipboardJob {
            paste: Some((layer, image)),
            result,
        });
        self.status = "reading clipboard".into();
    }

    fn paste(&mut self, layer: bool, image: bool) -> Result<(), String> {
        let clip = self
            .clipboard
            .as_ref()
            .ok_or("the openpaint clipboard is empty")?;
        if image {
            let mut doc = Document::from_rgba(
                clip.width,
                clip.height,
                clip.pixels.clone(),
                "Pasted image".into(),
            )?;
            doc.mark_unsaved();
            self.add_document(doc);
            return Ok(());
        }
        let doc = &mut self.tabs[self.active].document;
        if layer {
            doc.add_layer()?;
        }
        doc.begin_edit("Paste");
        let result = tools::paste(doc, clip.width, clip.height, &clip.pixels, (0, 0));
        if result.is_ok() {
            doc.finish_edit();
            self.tool = Tool::MovePixels;
        } else {
            doc.cancel_edit();
        }
        result
    }

    fn cancel_effect(&mut self) {
        self.effect_job = None;
        if let Some(preview) = self.effect_preview.take() {
            self.tabs[preview.tab]
                .document
                .active_pixels_mut()
                .copy_from_slice(&preview.original);
            self.tabs[preview.tab].dirty_texture = true;
        }
        self.status = "cancelled".into();
    }

    fn start_effect(&mut self, name: String, value: f32, second: f32, preview: bool) {
        self.start_adjustment(
            Adjustment::Basic {
                name,
                value,
                second,
            },
            preview,
        );
    }

    fn start_adjustment(&mut self, adjustment: Adjustment, preview: bool) {
        let doc = &self.tabs[self.active].document;
        if self.effect_preview.is_none() {
            self.effect_preview = Some(EffectPreview {
                tab: self.active,
                original: doc.active_pixels().to_vec(),
            });
        }
        let pixels = self.effect_preview.as_ref().unwrap().original.clone();
        let width = doc.width;
        let height = doc.height;
        let (sender, result) = mpsc::channel();
        let operation = adjustment.clone();
        std::thread::spawn(move || {
            let result = match operation {
                Adjustment::Basic {
                    name,
                    value,
                    second,
                } => effects::apply(&pixels, width, height, &name, value, second),
                Adjustment::Levels(settings) => {
                    effects::apply_levels(&pixels, width, height, &settings)
                }
                Adjustment::Curves(settings) => {
                    effects::apply_curves(&pixels, width, height, &settings)
                }
            };
            let _ = sender.send(result);
        });
        self.effect_job = Some(EffectJob {
            tab: self.active,
            label: adjustment.name().into(),
            preview,
            adjustment,
            result,
        });
        self.status = if preview {
            "rendering preview…".into()
        } else {
            "applying effect…".into()
        };
    }

    fn poll_jobs(&mut self) {
        if let Some(job) = &self.clipboard_job {
            if let Ok(result) = job.result.try_recv() {
                let paste = job.paste;
                self.clipboard_job = None;
                if let Some((layer, image)) = paste {
                    if let Ok(Some(data)) = result {
                        self.clipboard = Some(data);
                    }
                    if let Err(error) = self.paste(layer, image) {
                        self.error(error);
                    } else {
                        self.tabs[self.active].dirty_texture = true;
                        self.status = "pasted pixels".into();
                    }
                } else {
                    self.status = if result.is_ok() {
                        "copied pixels".into()
                    } else {
                        "copied within openpaint; system clipboard unavailable".into()
                    };
                }
            }
        }
        if let Some(job) = &self.file_dialog {
            if let Ok(result) = job.result.try_recv() {
                let save = job.save;
                let import_layer = job.import_layer;
                let tab = job.tab;
                self.file_dialog = None;
                match result {
                    Ok(Some(path)) => {
                        if save {
                            self.save_path(tab, path);
                        } else if import_layer {
                            self.import_path(tab, path);
                        } else {
                            self.open_path(path);
                        }
                    }
                    Ok(None) => {
                        self.cancel_save_all();
                        self.status = "cancelled".into();
                        self.pending_close = None;
                        self.closing_all = false;
                    }
                    Err(_) => {
                        if import_layer {
                            self.ui.show_import_path();
                        } else {
                            self.ui.show_path_dialog(
                                save,
                                if save {
                                    "Untitled.opaint".into()
                                } else {
                                    String::new()
                                },
                            );
                        }
                        self.status = "enter an image path".into();
                    }
                }
            }
        }
        if let Some(job) = &self.effect_job {
            if let Ok(result) = job.result.try_recv() {
                let tab = job.tab;
                let label = job.label.clone();
                let preview = job.preview;
                let adjustment = job.adjustment.clone();
                self.effect_job = None;
                match result {
                    Ok(pixels) => {
                        let doc = &mut self.tabs[tab].document;
                        if pixels.len() == doc.active_pixels().len() {
                            if let Some(original) = &self.effect_preview {
                                doc.active_pixels_mut().copy_from_slice(&original.original);
                            }
                            if !preview {
                                doc.begin_edit(&label);
                            }
                            let selection = doc.selection.clone();
                            let target = doc.active_pixels_mut();
                            for (index, (before, after)) in target
                                .chunks_exact_mut(4)
                                .zip(pixels.chunks_exact(4))
                                .enumerate()
                            {
                                let coverage =
                                    selection.as_ref().map_or(255, |mask| mask[index]) as u32;
                                for channel in 0..4 {
                                    before[channel] = ((before[channel] as u32 * (255 - coverage)
                                        + after[channel] as u32 * coverage
                                        + 127)
                                        / 255)
                                        as u8;
                                }
                            }
                            if !preview {
                                doc.finish_edit();
                                self.effect_preview = None;
                                self.last_adjustment = Some(adjustment);
                            }
                            self.tabs[tab].dirty_texture = true;
                            self.status = if preview {
                                "preview; apply to keep or cancel to restore".into()
                            } else {
                                "effect applied".into()
                            };
                        }
                    }
                    Err(error) => {
                        self.cancel_effect();
                        self.error(error);
                    }
                }
            }
        }
    }

    fn remember(&mut self, path: PathBuf) {
        let path = path.canonicalize().unwrap_or(path);
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(10);
        self.ui.set_recent(
            self.recent
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        );
        self.write_settings();
    }

    fn read_settings(&mut self) {
        let Some(path) = io::config_path() else {
            return;
        };
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return;
        };
        if let Some(recent) = value["recent"].as_array() {
            self.recent = recent
                .iter()
                .filter_map(|p| p.as_str().map(PathBuf::from))
                .take(10)
                .collect();
        }
        self.ui.set_recent(
            self.recent
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        );
    }

    fn write_settings(&self) {
        use std::io::Write;
        let Some(path) = io::config_path() else {
            return;
        };
        let Some(parent) = path.parent() else { return };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let recent: Vec<_> = self.recent.iter().map(|p| p.to_string_lossy()).collect();
        if let Ok(bytes) = serde_json::to_vec_pretty(&serde_json::json!({"recent":recent})) {
            let _ = io::atomic_write(&path, |file| {
                file.write_all(&bytes).map_err(|e| e.to_string())
            });
        }
    }

    fn error(&mut self, error: String) {
        self.status = error.clone();
        self.ui.show_message(&error);
    }
}

fn ctrl() -> bool {
    is_key_down(KeyCode::LeftControl)
        || is_key_down(KeyCode::RightControl)
        || is_key_down(KeyCode::LeftSuper)
        || is_key_down(KeyCode::RightSuper)
}
fn shift() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}
fn alt() -> bool {
    is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt)
}
fn selection_mode(default: SelectionMode, right: bool) -> SelectionMode {
    if ctrl() {
        if right {
            SelectionMode::Xor
        } else {
            SelectionMode::Union
        }
    } else if alt() {
        if right {
            SelectionMode::Intersect
        } else {
            SelectionMode::Subtract
        }
    } else {
        default
    }
}
fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > x && bottom > y).then(|| Rect::new(x, y, right - x, bottom - y))
}

fn drag_transform(
    drag: &DragTransform,
    point: (f32, f32),
    constrain: bool,
) -> Option<tools::SelectionTransform> {
    match *drag {
        DragTransform::Move => None,
        DragTransform::Rotate { center, initial } => {
            let mut angle = (point.1 - center.1).atan2(point.0 - center.0) - initial;
            if constrain {
                let step = std::f32::consts::PI / 12.0;
                angle = (angle / step).round() * step;
            }
            Some(tools::SelectionTransform {
                center,
                angle,
                ..Default::default()
            })
        }
        DragTransform::Scale { anchor, initial } => {
            let mut x = if initial.0.abs() > 0.001 {
                (point.0 - anchor.0) / initial.0
            } else {
                1.0
            };
            let mut y = if initial.1.abs() > 0.001 {
                (point.1 - anchor.1) / initial.1
            } else {
                1.0
            };
            if constrain {
                let size = x.abs().max(y.abs());
                x = size * x.signum();
                y = size * y.signum();
            }
            Some(tools::SelectionTransform {
                center: anchor,
                scale: (x, y),
                ..Default::default()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Editor {
        let mut editor = Editor::blank();
        editor.tabs = vec![Tab::new(
            Document::from_rgba(
                2,
                1,
                vec![20, 40, 60, 255, 80, 100, 120, 255],
                "test".into(),
            )
            .unwrap(),
        )];
        editor
    }

    fn wait_effect(editor: &mut Editor) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while editor.effect_job.is_some() && std::time::Instant::now() < deadline {
            editor.poll_jobs();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(editor.effect_job.is_none(), "effect worker did not finish");
    }

    #[test]
    fn repeated_preview_is_not_compounded_and_cancel_restores_history() {
        let mut editor = editor();
        let before = editor.tabs[0].document.active_pixels().to_vec();
        editor.start_effect("invert".into(), 0.0, 0.0, true);
        wait_effect(&mut editor);
        let preview = editor.tabs[0].document.active_pixels().to_vec();
        assert_ne!(preview, before);
        editor.start_effect("invert".into(), 0.0, 0.0, true);
        wait_effect(&mut editor);
        assert_eq!(editor.tabs[0].document.active_pixels(), preview);
        editor.cancel_effect();
        assert_eq!(editor.tabs[0].document.active_pixels(), before);
        assert_eq!(editor.tabs[0].document.history_labels().len(), 1);
        assert!(!editor.tabs[0].document.is_dirty());
    }

    #[test]
    fn effect_apply_is_one_undo_step_and_respects_selection() {
        let mut editor = editor();
        let before = editor.tabs[0].document.active_pixels().to_vec();
        editor.tabs[0].document.selection = Some(vec![255, 0]);
        editor.start_effect("invert".into(), 0.0, 0.0, true);
        wait_effect(&mut editor);
        editor.start_effect("invert".into(), 0.0, 0.0, false);
        wait_effect(&mut editor);
        assert_eq!(
            &editor.tabs[0].document.active_pixels()[..4],
            &[235, 215, 195, 255]
        );
        assert_eq!(&editor.tabs[0].document.active_pixels()[4..], &before[4..]);
        assert_eq!(editor.tabs[0].document.history_labels().len(), 2);
        editor.action(Action::Undo);
        assert_eq!(editor.tabs[0].document.active_pixels(), before);
        assert!(!editor.tabs[0].document.is_dirty());
    }

    #[test]
    fn cancel_running_effect_discards_its_late_result() {
        let mut editor = editor();
        let before = editor.tabs[0].document.active_pixels().to_vec();
        editor.start_effect("invert".into(), 0.0, 0.0, true);
        editor.cancel_effect();
        editor.poll_jobs();
        assert_eq!(editor.tabs[0].document.active_pixels(), before);
        assert!(editor.effect_job.is_none());
    }

    #[test]
    fn new_tabs_are_distinct_and_undo_is_per_document() {
        let mut editor = editor();
        editor.action(Action::AddLayer);
        editor.action(Action::New {
            width: 3,
            height: 4,
        });
        assert_eq!(editor.tabs.len(), 2);
        assert_eq!(editor.active, 1);
        editor.action(Action::Undo);
        assert_eq!(editor.tabs[0].document.layers.len(), 2);
        editor.action(Action::SwitchTab(0));
        editor.action(Action::Undo);
        assert_eq!(editor.tabs[0].document.layers.len(), 1);
    }

    #[test]
    fn dirty_close_can_cancel_without_losing_the_document() {
        let mut editor = editor();
        editor.action(Action::AddLayer);
        editor.request_close(0);
        assert!(editor.ui.has_modal());
        assert_eq!(editor.pending_close, Some(0));
        editor.action(Action::ConfirmCancel);
        assert!(editor.pending_close.is_none());
        assert_eq!(editor.tabs[0].document.layers.len(), 2);
    }

    #[test]
    fn paste_as_image_is_unsaved_and_requires_close_confirmation() {
        let mut editor = editor();
        editor.clipboard = Some(ImageData {
            width: 1,
            height: 1,
            pixels: vec![1, 2, 3, 255],
        });
        editor.paste(false, true).unwrap();
        assert!(editor.tabs[editor.active].document.is_dirty());
        editor.request_close(editor.active);
        assert!(editor.pending_close.is_some());
    }
}
