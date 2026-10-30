use macroquad::prelude::*;
use openpaint::document::{BlendMode, Document, Pixel};
use openpaint::tools::{GradientKind, SelectionMode, Tool, ToolSettings};

const FACE: Color = color_u8!(44, 47, 51, 255);
const PANEL: Color = color_u8!(36, 38, 42, 255);
const FIELD: Color = color_u8!(28, 30, 33, 255);
const HOVER: Color = color_u8!(54, 61, 69, 255);
const INK: Color = color_u8!(230, 233, 237, 255);
const MUTED: Color = color_u8!(154, 161, 171, 255);
const ACCENT: Color = color_u8!(111, 171, 228, 255);
const SELECTED: Color = color_u8!(49, 73, 98, 255);
const ERROR: Color = color_u8!(255, 143, 137, 255);
pub(super) const BORDER: Color = color_u8!(76, 82, 90, 255);
pub(super) const WORKSPACE: Color = color_u8!(25, 27, 30, 255);
pub(super) const CHECKER_LIGHT: Color = color_u8!(88, 93, 101, 255);
pub(super) const CHECKER_DARK: Color = color_u8!(65, 69, 76, 255);

#[derive(Clone, Debug)]
pub enum Action {
    New {
        width: u32,
        height: u32,
    },
    Open,
    OpenRecent(usize),
    Save,
    SaveAs,
    OpenPath(String),
    SavePath(String),
    CloseTab(usize),
    SwitchTab(usize),
    Quit,
    Undo,
    Redo,
    SelectAll,
    Deselect,
    InvertSelection,
    DeleteSelection,
    Copy,
    Cut,
    Paste,
    PasteNewLayer,
    PasteNewImage,
    Tool(Tool),
    Settings(ToolSettings),
    Primary(Pixel),
    Secondary(Pixel),
    SwapColors,
    ResetColors,
    Zoom(f32),
    Fit,
    ActualSize,
    AddLayer,
    DeleteLayer,
    DuplicateLayer,
    MergeDown,
    Flatten,
    MoveLayer(i32),
    SelectLayer(usize),
    ToggleLayer(usize),
    LayerProperties {
        name: String,
        opacity: u8,
        blend: BlendMode,
    },
    Resize {
        width: u32,
        height: u32,
        canvas: bool,
    },
    Crop,
    FlipHorizontal,
    FlipVertical,
    RotateCw,
    RotateCcw,
    Text {
        text: String,
        size: f32,
    },
    Effect {
        name: String,
        value: f32,
        second: f32,
    },
    PreviewEffect {
        name: String,
        value: f32,
        second: f32,
    },
    CancelEffect,
    ConfirmSave,
    ConfirmDiscard,
    ConfirmCancel,
}

pub struct Model<'a> {
    pub doc: &'a Document,
    pub tabs: &'a [(String, bool)],
    pub thumbnails: &'a [Option<Texture2D>],
    pub active: usize,
    pub tool: Tool,
    pub settings: &'a ToolSettings,
    pub primary: Pixel,
    pub secondary: Pixel,
    pub zoom: f32,
    pub status: &'a str,
    pub busy: bool,
}

pub struct Output {
    pub viewport: Rect,
    pub actions: Vec<Action>,
    pub keyboard_captured: bool,
    pub pointer_captured: bool,
}

enum ModalKind {
    New,
    Resize(bool),
    Layer(usize),
    Text,
    Effect(String),
    Path(bool),
    Color(bool),
    Confirm,
    Message,
}

struct Field {
    label: String,
    value: String,
    edit: TextEdit,
}

#[derive(Default)]
struct TextEdit {
    cursor: usize,
    anchor: usize,
    scroll_x: f32,
    first_line: usize,
}

struct Modal {
    title: String,
    message: String,
    fields: Vec<Field>,
    kind: ModalKind,
    error: String,
}

impl Modal {
    fn new(title: &str, kind: ModalKind, fields: &[(&str, String)]) -> Self {
        Self {
            title: title.into(),
            message: String::new(),
            fields: fields
                .iter()
                .map(|(label, value)| Field {
                    label: (*label).into(),
                    value: value.clone(),
                    edit: TextEdit {
                        cursor: value.len(),
                        ..TextEdit::default()
                    },
                })
                .collect(),
            kind,
            error: String::new(),
        }
    }
}

enum MenuCommand {
    Action(Action),
    New,
    Resize(bool),
    Properties,
    Text,
    Effect(&'static str, f32, f32),
    Panel(usize),
    About,
    Separator,
}

struct MenuItem {
    label: String,
    shortcut: &'static str,
    command: MenuCommand,
    enabled: bool,
}

impl MenuItem {
    fn new(label: &str, shortcut: &'static str, command: MenuCommand) -> Self {
        Self {
            label: label.into(),
            shortcut,
            command,
            enabled: true,
        }
    }
    fn action(label: &str, shortcut: &'static str, action: Action) -> Self {
        Self::new(label, shortcut, MenuCommand::Action(action))
    }
    fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }
    fn separator() -> Self {
        Self::new("", "", MenuCommand::Separator)
    }
}

pub struct Ui {
    menu: Option<usize>,
    menu_cursor: Option<usize>,
    modal: Option<Modal>,
    focus: Option<usize>,
    text_drag: Option<usize>,
    panels: [bool; 4],
    panel_positions: [Option<Vec2>; 4],
    panel_order: [usize; 4],
    panel_drag: Option<(usize, Vec2)>,
    color_secondary: bool,
    recent: Vec<String>,
    layer_scroll: usize,
    history_scroll: usize,
    tab_scroll: usize,
    last_active: usize,
    last_history: usize,
    hover: Option<(String, Vec2)>,
    last_mouse: Vec2,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        Self {
            menu: None,
            menu_cursor: None,
            modal: None,
            focus: None,
            text_drag: None,
            panels: [true; 4],
            panel_positions: [None; 4],
            panel_order: [0, 3, 1, 2],
            panel_drag: None,
            color_secondary: false,
            recent: Vec::new(),
            layer_scroll: 0,
            history_scroll: 0,
            tab_scroll: 0,
            last_active: usize::MAX,
            last_history: usize::MAX,
            hover: None,
            last_mouse: vec2(-1.0, -1.0),
        }
    }

    pub fn set_recent(&mut self, recent: Vec<String>) {
        self.recent = recent;
    }
    pub fn has_modal(&self) -> bool {
        self.modal.is_some()
    }
    pub fn viewport(&self) -> Rect {
        let left = if self.panels[0] || self.panels[3] {
            184.0
        } else {
            8.0
        };
        let right = if self.panels[1] || self.panels[2] {
            228.0
        } else {
            8.0
        };
        Rect::new(
            left,
            143.0,
            (screen_width() - left - right).max(40.0),
            (screen_height() - 171.0).max(40.0),
        )
    }
    fn panel_rect(&self, index: usize) -> Rect {
        let history_height = if self.panels[2] {
            ((screen_height() - 185.0) * 0.42).clamp(125.0, 230.0)
        } else {
            (screen_height() - 180.0).max(100.0)
        };
        let default = match index {
            0 => Rect::new(8.0, 144.0, 167.0, 229.0),
            1 => Rect::new(screen_width() - 219.0, 144.0, 210.0, history_height),
            2 => {
                let y = if self.panels[1] {
                    154.0 + history_height
                } else {
                    144.0
                };
                Rect::new(
                    screen_width() - 219.0,
                    y,
                    210.0,
                    (screen_height() - y - 33.0).max(100.0),
                )
            }
            _ => Rect::new(8.0, (screen_height() - 309.0).max(144.0), 167.0, 276.0),
        };
        let position = self.panel_positions[index].unwrap_or(vec2(default.x, default.y));
        Rect::new(
            position.x.clamp(0.0, (screen_width() - default.w).max(0.0)),
            position.y.clamp(137.0, (screen_height() - 48.0).max(137.0)),
            default.w,
            default.h,
        )
    }
    fn hovered_panel(&self) -> Option<usize> {
        self.panel_order
            .iter()
            .rev()
            .copied()
            .find(|&index| self.panels[index] && self.panel_rect(index).contains(mouse()))
    }
    fn move_panels(&mut self, interactive: bool) {
        if let Some((index, offset)) = self.panel_drag {
            if is_mouse_button_down(MouseButton::Left) && interactive {
                let rect = self.panel_rect(index);
                let p = mouse() - offset;
                self.panel_positions[index] = Some(vec2(
                    p.x.clamp(0.0, (screen_width() - rect.w).max(0.0)),
                    p.y.clamp(137.0, (screen_height() - 48.0).max(137.0)),
                ));
            } else {
                self.panel_drag = None;
            }
        } else if interactive && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(index) = self.hovered_panel() {
                let rect = self.panel_rect(index);
                if let Some(order) = self.panel_order.iter().position(|&item| item == index) {
                    self.panel_order[order..].rotate_left(1);
                }
                if Rect::new(rect.x, rect.y, rect.w - 24.0, 23.0).contains(mouse()) {
                    self.panel_drag = Some((index, mouse() - vec2(rect.x, rect.y)));
                }
            }
        }
    }
    fn menu_keys(&mut self, model: &Model<'_>) {
        let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
        if alt {
            for (index, key) in [
                KeyCode::F,
                KeyCode::E,
                KeyCode::V,
                KeyCode::I,
                KeyCode::L,
                KeyCode::A,
                KeyCode::C,
                KeyCode::W,
                KeyCode::H,
            ]
            .into_iter()
            .enumerate()
            {
                if is_key_pressed(key) {
                    self.menu = Some(index);
                    self.menu_cursor = None;
                }
            }
        }
        if is_key_pressed(KeyCode::F10) {
            self.menu = if self.menu.is_some() { None } else { Some(0) };
            self.menu_cursor = None;
        }
        if let Some(index) = self.menu {
            if is_key_pressed(KeyCode::Left) {
                self.menu = Some((index + 8) % 9);
                self.menu_cursor = None;
            }
            if is_key_pressed(KeyCode::Right) {
                self.menu = Some((index + 1) % 9);
                self.menu_cursor = None;
            }
            if let Some(menu) = self.menu {
                let items = self.menu_items(menu, model);
                let eligible: Vec<usize> = items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        item.enabled && !matches!(item.command, MenuCommand::Separator)
                    })
                    .map(|(index, _)| index)
                    .collect();
                if is_key_pressed(KeyCode::Down) {
                    self.menu_cursor = next_menu_item(&eligible, self.menu_cursor, false);
                } else if is_key_pressed(KeyCode::Up) {
                    self.menu_cursor = next_menu_item(&eligible, self.menu_cursor, true);
                } else if self.menu_cursor.is_none() {
                    self.menu_cursor = eligible.first().copied();
                }
            }
        }
    }
    fn set_modal(&mut self, modal: Modal) {
        self.menu = None;
        self.focus = (!modal.fields.is_empty()).then_some(0);
        self.text_drag = None;
        self.modal = Some(modal);
    }
    pub fn show_new(&mut self, width: u32, height: u32) {
        self.set_modal(Modal::new(
            "New",
            ModalKind::New,
            &[
                ("Width (pixels)", width.to_string()),
                ("Height (pixels)", height.to_string()),
            ],
        ));
    }
    pub fn show_resize(&mut self, width: u32, height: u32, canvas: bool) {
        self.set_modal(Modal::new(
            if canvas { "Canvas Size" } else { "Resize" },
            ModalKind::Resize(canvas),
            &[
                ("Width (pixels)", width.to_string()),
                ("Height (pixels)", height.to_string()),
            ],
        ));
    }
    pub fn show_layer_properties(&mut self, doc: &Document) {
        let layer = &doc.layers[doc.active_layer];
        let mode = BlendMode::ALL
            .iter()
            .position(|m| *m == layer.blend_mode)
            .unwrap_or(0);
        self.set_modal(Modal::new(
            "Layer Properties",
            ModalKind::Layer(mode),
            &[
                ("Name", layer.name.clone()),
                ("Opacity (0–255)", layer.opacity.to_string()),
            ],
        ));
    }
    pub fn show_text(&mut self) {
        self.set_modal(Modal::new(
            "Text",
            ModalKind::Text,
            &[("Text", String::new()), ("Size (pixels)", "24".into())],
        ));
    }
    pub fn show_effect(&mut self, name: &str, value: f32, second: f32) {
        let (title, first, next) = effect_labels(name);
        let mut fields = Vec::new();
        if !first.is_empty() {
            fields.push((first, value.to_string()));
        }
        if !next.is_empty() {
            fields.push((next, second.to_string()));
        }
        self.set_modal(Modal::new(title, ModalKind::Effect(name.into()), &fields));
    }
    pub fn show_confirm(&mut self, message: &str) {
        let mut modal = Modal::new("Unsaved changes", ModalKind::Confirm, &[]);
        modal.message = message.into();
        self.set_modal(modal);
    }
    pub fn show_path_dialog(&mut self, save: bool, initial: String) {
        let mut modal = Modal::new(
            if save { "Save As" } else { "Open" },
            ModalKind::Path(save),
            &[("File path", initial)],
        );
        modal.message = "use .opaint to preserve layers, or .png to export an image.".into();
        self.set_modal(modal);
    }
    pub fn show_message(&mut self, message: &str) {
        let mut modal = Modal::new("openpaint", ModalKind::Message, &[]);
        modal.message = message.into();
        self.set_modal(modal);
    }

    pub fn draw(&mut self, model: Model<'_>) -> Output {
        let mut actions = Vec::new();
        self.hover = None;
        let started_modal = self.modal.is_some();
        if !started_modal && !model.busy {
            self.menu_keys(&model);
        }
        let started_menu = self.menu.is_some();
        if !started_modal && !model.busy {
            for (key, panel) in [
                (KeyCode::F5, 0),
                (KeyCode::F6, 1),
                (KeyCode::F7, 2),
                (KeyCode::F8, 3),
            ] {
                if is_key_pressed(key) {
                    self.panels[panel] = !self.panels[panel];
                }
            }
        }
        let interactive = !started_modal && !model.busy;
        let dragging_panel = self.panel_drag.is_some();
        self.move_panels(interactive && !started_menu);
        self.menu_bar(&model, interactive, &mut actions);
        self.toolbar(&model, interactive && !started_menu, &mut actions);
        self.tool_options(&model, interactive && !started_menu, &mut actions);
        self.tabs(&model, interactive && !started_menu, &mut actions);
        let viewport = self.viewport();
        let hovered_panel = self.hovered_panel();
        for index in self.panel_order {
            if self.panels[index] {
                let panel_interactive = interactive
                    && !started_menu
                    && hovered_panel == Some(index)
                    && self.panel_drag.is_none();
                match index {
                    0 => self.tools_panel(&model, panel_interactive, &mut actions),
                    1 => self.history_panel(&model, panel_interactive, &mut actions),
                    2 => self.layers_panel(&model, panel_interactive, &mut actions),
                    _ => self.colors_panel(&model, panel_interactive, &mut actions),
                }
            }
        }
        self.status_bar(&model, interactive && !started_menu, &mut actions);
        if let Some(menu) = self.menu {
            self.draw_menu(menu, &model, &mut actions);
        }
        if self.modal.is_some() {
            self.draw_modal(&mut actions, model.busy || !started_modal);
        }
        if model.busy {
            let r = Rect::new(screen_width() / 2.0 - 110.0, 150.0, 220.0, 35.0);
            panel(r, "");
            text("working…", r.x + 12.0, r.y + 23.0, 15.0, INK);
        }
        if self.menu.is_none() && self.modal.is_none() {
            if let Some((label, point)) = &self.hover {
                let width = measure_text(label, None, 13, 1.0).width + 14.0;
                let r = Rect::new(
                    point.x.min(screen_width() - width - 5.0),
                    (point.y + 23.0).min(screen_height() - 30.0),
                    width,
                    22.0,
                );
                draw_rectangle(r.x, r.y, r.w, r.h, HOVER);
                draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
                text(label, r.x + 7.0, r.y + 15.0, 13.0, INK);
            }
        }
        let overlay = started_modal
            || started_menu
            || self.modal.is_some()
            || self.menu.is_some()
            || model.busy;
        self.last_mouse = mouse();
        while get_char_pressed().is_some() {}
        Output {
            viewport,
            actions,
            keyboard_captured: overlay || dragging_panel || self.panel_drag.is_some(),
            pointer_captured: overlay
                || dragging_panel
                || self.panel_drag.is_some()
                || hovered_panel.is_some()
                || !viewport.contains(mouse()),
        }
    }

    fn menu_bar(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        draw_rectangle(0.0, 0.0, screen_width(), 28.0, FACE);
        let labels = [
            "File",
            "Edit",
            "View",
            "Image",
            "Layers",
            "Adjustments",
            "Effects",
            "Window",
            "Help",
        ];
        let mut x = 6.0;
        for (index, label) in labels.iter().enumerate() {
            let width = menu_width(label);
            let r = Rect::new(x, 2.0, width, 24.0);
            if button(r, label, self.menu == Some(index), interactive) {
                self.menu = if self.menu == Some(index) {
                    None
                } else {
                    Some(index)
                };
                self.menu_cursor = None;
            }
            if interactive
                && self.menu.is_some()
                && r.contains(mouse())
                && mouse() != self.last_mouse
                && !is_mouse_button_pressed(MouseButton::Left)
            {
                self.menu = Some(index);
                self.menu_cursor = None;
            }
            x += width;
        }
        text("openpaint", screen_width() - 87.0, 19.0, 14.0, MUTED);
        let _ = (model, actions);
    }

    fn toolbar(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        draw_rectangle(0.0, 28.0, screen_width(), 35.0, FACE);
        let mut x = 8.0;
        for (label, action) in [
            ("New", None),
            ("Open", Some(Action::Open)),
            ("Save", Some(Action::Save)),
            ("Undo", Some(Action::Undo)),
            ("Redo", Some(Action::Redo)),
            ("Cut", Some(Action::Cut)),
            ("Copy", Some(Action::Copy)),
            ("Paste", Some(Action::Paste)),
        ] {
            let enabled = match &action {
                Some(Action::Undo) => model.doc.can_undo(),
                Some(Action::Redo) => model.doc.can_redo(),
                _ => true,
            };
            let r = Rect::new(x, 32.0, if label == "Paste" { 52.0 } else { 47.0 }, 26.0);
            if button(r, label, false, interactive && enabled) {
                if let Some(action) = action {
                    actions.push(action);
                } else {
                    self.show_new(800, 600);
                }
            }
            x += r.w + 3.0;
        }
        draw_line(x + 4.0, 35.0, x + 4.0, 56.0, 1.0, BORDER);
        x += 16.0;
        if button(Rect::new(x, 32.0, 37.0, 26.0), "−", false, interactive) {
            actions.push(Action::Zoom(model.zoom / 1.25));
        }
        text(
            &format!("{}%", (model.zoom * 100.0).round()),
            x + 44.0,
            50.0,
            14.0,
            INK,
        );
        if button(
            Rect::new(x + 97.0, 32.0, 34.0, 26.0),
            "+",
            false,
            interactive,
        ) {
            actions.push(Action::Zoom(model.zoom * 1.25));
        }
        if button(
            Rect::new(x + 136.0, 32.0, 42.0, 26.0),
            "Fit",
            false,
            interactive,
        ) {
            actions.push(Action::Fit);
        }
        if button(
            Rect::new(x + 182.0, 32.0, 46.0, 26.0),
            "100%",
            false,
            interactive,
        ) {
            actions.push(Action::ActualSize);
        }
        let px = (screen_width() - 222.0).max(x + 245.0);
        for (index, label) in ["Tools", "History", "Layers", "Colors"].iter().enumerate() {
            let order = [0, 1, 2, 3][index];
            if button(
                Rect::new(px + index as f32 * 53.0, 32.0, 51.0, 26.0),
                label,
                self.panels[order],
                interactive,
            ) {
                self.panels[order] = !self.panels[order];
            }
        }
    }

    fn tool_options(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        draw_rectangle(0.0, 63.0, screen_width(), 35.0, PANEL);
        draw_line(0.0, 63.0, screen_width(), 63.0, 1.0, BORDER);
        text(model.tool.name(), 12.0, 85.0, 14.0, INK);
        let mut settings = model.settings.clone();
        let mut changed = false;
        let mut x = 193.0;
        if matches!(
            model.tool,
            Tool::RectangleSelect | Tool::EllipseSelect | Tool::Lasso | Tool::MagicWand
        ) {
            let modes = [
                SelectionMode::Replace,
                SelectionMode::Union,
                SelectionMode::Subtract,
                SelectionMode::Intersect,
                SelectionMode::Xor,
            ];
            let labels = ["Replace", "Add", "Subtract", "Intersect", "Xor"];
            for (mode, label) in modes.into_iter().zip(labels) {
                let width = menu_width(label);
                if button(
                    Rect::new(x, 68.0, width, 25.0),
                    label,
                    settings.selection_mode == mode,
                    interactive,
                ) {
                    settings.selection_mode = mode;
                    changed = true;
                }
                x += width + 2.0;
            }
        } else if matches!(
            model.tool,
            Tool::Paintbrush
                | Tool::Pencil
                | Tool::Eraser
                | Tool::CloneStamp
                | Tool::Recolor
                | Tool::Line
                | Tool::Rectangle
                | Tool::Ellipse
                | Tool::RoundedRectangle
        ) {
            text("Width:", x, 85.0, 13.0, INK);
            x += 43.0;
            if button(Rect::new(x, 68.0, 24.0, 25.0), "−", false, interactive) {
                settings.size = (settings.size - 1.0).max(1.0);
                changed = true;
            }
            text(
                &format!("{}", settings.size.round()),
                x + 30.0,
                85.0,
                13.0,
                INK,
            );
            if button(
                Rect::new(x + 64.0, 68.0, 24.0, 25.0),
                "+",
                false,
                interactive,
            ) {
                settings.size = (settings.size + 1.0).min(500.0);
                changed = true;
            }
            x += 102.0;
            if matches!(
                model.tool,
                Tool::Paintbrush | Tool::Eraser | Tool::CloneStamp | Tool::Recolor
            ) {
                text("Hardness:", x, 85.0, 13.0, INK);
                if let Some(value) = slider(
                    Rect::new(x + 67.0, 71.0, 85.0, 17.0),
                    settings.hardness,
                    interactive,
                ) {
                    settings.hardness = value;
                    changed = true;
                }
                x += 169.0;
            }
        }
        if matches!(
            model.tool,
            Tool::PaintBucket | Tool::MagicWand | Tool::Recolor
        ) {
            text("Tolerance:", x, 85.0, 13.0, INK);
            if let Some(value) = slider(
                Rect::new(x + 72.0, 71.0, 100.0, 17.0),
                settings.tolerance as f32 / 255.0,
                interactive,
            ) {
                settings.tolerance = (value * 255.0).round() as u8;
                changed = true;
            }
            x += 183.0;
            if button(
                Rect::new(x, 68.0, 90.0, 25.0),
                if settings.contiguous {
                    "Contiguous"
                } else {
                    "Global"
                },
                settings.contiguous,
                interactive,
            ) {
                settings.contiguous = !settings.contiguous;
                changed = true;
            }
            x += 102.0;
        }
        if model.tool == Tool::Gradient {
            for (kind, label) in [
                (GradientKind::Linear, "Linear"),
                (GradientKind::Reflected, "Reflected"),
                (GradientKind::Radial, "Radial"),
                (GradientKind::Diamond, "Diamond"),
                (GradientKind::Conical, "Conical"),
            ] {
                let width = menu_width(label);
                if button(
                    Rect::new(x, 68.0, width, 25.0),
                    label,
                    settings.gradient == kind,
                    interactive,
                ) {
                    settings.gradient = kind;
                    changed = true;
                }
                x += width + 2.0;
            }
        }
        if matches!(
            model.tool,
            Tool::Rectangle | Tool::Ellipse | Tool::RoundedRectangle
        ) {
            if button(
                Rect::new(x, 68.0, 58.0, 25.0),
                "Outline",
                settings.outlined,
                interactive,
            ) {
                settings.outlined = !settings.outlined;
                if !settings.outlined {
                    settings.filled = true;
                }
                changed = true;
            }
            if button(
                Rect::new(x + 62.0, 68.0, 39.0, 25.0),
                "Fill",
                settings.filled,
                interactive,
            ) {
                settings.filled = !settings.filled;
                if !settings.filled {
                    settings.outlined = true;
                }
                changed = true;
            }
            x += 115.0;
        }
        if matches!(
            model.tool,
            Tool::Paintbrush
                | Tool::Eraser
                | Tool::Pencil
                | Tool::CloneStamp
                | Tool::PaintBucket
                | Tool::Gradient
                | Tool::Recolor
                | Tool::Line
                | Tool::Rectangle
                | Tool::Ellipse
                | Tool::RoundedRectangle
        ) && x + 180.0 < screen_width()
        {
            text("Opacity:", x, 85.0, 13.0, INK);
            if let Some(value) = slider(
                Rect::new(x + 59.0, 71.0, 95.0, 17.0),
                settings.opacity as f32 / 255.0,
                interactive,
            ) {
                settings.opacity = (value * 255.0).round() as u8;
                changed = true;
            }
        }
        if model.tool == Tool::Text
            && button(
                Rect::new(x, 68.0, 97.0, 25.0),
                "Enter text…",
                false,
                interactive,
            )
        {
            self.show_text();
        }
        if changed {
            actions.push(Action::Settings(settings));
        }
        draw_line(0.0, 98.0, screen_width(), 98.0, 1.0, BORDER);
    }

    fn tabs(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        draw_rectangle(0.0, 99.0, screen_width(), 38.0, WORKSPACE);
        let count = ((screen_width() - 70.0) / 170.0).floor().max(1.0) as usize;
        self.tab_scroll = self.tab_scroll.min(model.tabs.len().saturating_sub(count));
        if model.active != self.last_active {
            if model.active < self.tab_scroll {
                self.tab_scroll = model.active;
            }
            if model.active >= self.tab_scroll + count {
                self.tab_scroll = model.active + 1 - count;
            }
            self.last_active = model.active;
            self.last_history = usize::MAX;
        }
        for (index, (name, dirty)) in model
            .tabs
            .iter()
            .enumerate()
            .skip(self.tab_scroll)
            .take(count)
        {
            let r = Rect::new(
                6.0 + (index - self.tab_scroll) as f32 * 170.0,
                102.0,
                165.0,
                32.0,
            );
            draw_rectangle(
                r.x,
                r.y,
                r.w,
                r.h,
                if index == model.active { PANEL } else { FACE },
            );
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
            let mini = Rect::new(r.x + 5.0, r.y + 5.0, 26.0, 22.0);
            checker(mini, 5.0);
            if let Some(Some(texture)) = model.thumbnails.get(index) {
                let scale = (mini.w / texture.width()).min(mini.h / texture.height());
                let size = vec2(texture.width() * scale, texture.height() * scale);
                draw_texture_ex(
                    texture,
                    mini.x + (mini.w - size.x) / 2.0,
                    mini.y + (mini.h - size.y) / 2.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(size),
                        ..Default::default()
                    },
                );
            }
            draw_rectangle_lines(mini.x, mini.y, mini.w, mini.h, 1.0, MUTED);
            let title = if *dirty {
                format!("{name} *")
            } else {
                name.clone()
            };
            clipped_text(&title, r.x + 37.0, r.y + 21.0, 102.0, 13.0, INK);
            if button(
                Rect::new(r.x + 144.0, r.y + 6.0, 17.0, 20.0),
                "×",
                false,
                interactive,
            ) {
                actions.push(Action::CloseTab(index));
            } else if interactive && clicked(r) {
                actions.push(Action::SwitchTab(index));
            }
        }
        if model.tabs.len() > count {
            let x = screen_width() - 57.0;
            if button(Rect::new(x, 105.0, 23.0, 25.0), "‹", false, interactive)
                && self.tab_scroll > 0
            {
                self.tab_scroll -= 1;
            }
            if button(
                Rect::new(x + 26.0, 105.0, 23.0, 25.0),
                "›",
                false,
                interactive,
            ) {
                self.tab_scroll = (self.tab_scroll + 1).min(model.tabs.len() - count);
            }
        }
    }

    fn tools_panel(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        let r = self.panel_rect(0);
        panel(r, "Tools");
        if close_button(r, interactive) {
            self.panels[0] = false;
        }
        for (index, tool) in Tool::ALL.into_iter().enumerate() {
            let cell = Rect::new(
                r.x + 9.0 + (index % 4) as f32 * 37.0,
                r.y + 29.0 + (index / 4) as f32 * 30.0,
                33.0,
                27.0,
            );
            if icon_button(cell, tool, model.tool == tool, interactive) {
                actions.push(Action::Tool(tool));
            }
            if interactive && cell.contains(mouse()) {
                self.hover = Some((tool.name().into(), mouse()));
            }
        }
        clipped_text(
            model.tool.name(),
            r.x + 8.0,
            r.y + r.h - 9.0,
            r.w - 16.0,
            12.0,
            MUTED,
        );
    }

    fn colors_panel(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        let r = self.panel_rect(3);
        panel(r, "Colors");
        if close_button(r, interactive) {
            self.panels[3] = false;
        }
        let secondary = Rect::new(r.x + 28.0, r.y + 44.0, 36.0, 31.0);
        let primary = Rect::new(r.x + 12.0, r.y + 31.0, 36.0, 31.0);
        swatch(secondary, model.secondary, self.color_secondary);
        swatch(primary, model.primary, !self.color_secondary);
        if interactive && clicked(primary) {
            self.color_secondary = false;
        } else if interactive && clicked(secondary) {
            self.color_secondary = true;
        }
        if button(
            Rect::new(r.x + 76.0, r.y + 32.0, 34.0, 25.0),
            "⇄",
            false,
            interactive,
        ) {
            actions.push(Action::SwapColors);
        }
        if button(
            Rect::new(r.x + 114.0, r.y + 32.0, 40.0, 25.0),
            "B/W",
            false,
            interactive,
        ) {
            actions.push(Action::ResetColors);
        }
        text(
            if self.color_secondary {
                "Secondary"
            } else {
                "Primary"
            },
            r.x + 76.0,
            r.y + 73.0,
            12.0,
            INK,
        );
        let colors: [Pixel; 32] = [
            [0, 0, 0, 255],
            [96, 96, 96, 255],
            [160, 160, 160, 255],
            [220, 220, 220, 255],
            [255, 255, 255, 255],
            [128, 0, 0, 255],
            [255, 0, 0, 255],
            [255, 128, 0, 255],
            [128, 64, 0, 255],
            [255, 192, 128, 255],
            [255, 255, 0, 255],
            [128, 128, 0, 255],
            [0, 128, 0, 255],
            [0, 192, 0, 255],
            [128, 255, 0, 255],
            [128, 255, 128, 255],
            [0, 128, 128, 255],
            [0, 255, 255, 255],
            [128, 255, 255, 255],
            [0, 64, 128, 255],
            [0, 128, 255, 255],
            [0, 0, 255, 255],
            [128, 128, 255, 255],
            [0, 0, 128, 255],
            [128, 0, 128, 255],
            [192, 0, 192, 255],
            [255, 0, 255, 255],
            [255, 128, 255, 255],
            [255, 128, 128, 255],
            [192, 160, 128, 255],
            [128, 96, 64, 255],
            [0, 0, 0, 0],
        ];
        for (index, color) in colors.into_iter().enumerate() {
            let cell = Rect::new(
                r.x + 10.0 + (index % 8) as f32 * 18.0,
                r.y + 84.0 + (index / 8) as f32 * 18.0,
                16.0,
                16.0,
            );
            swatch(cell, color, false);
            if interactive && cell.contains(mouse()) {
                if is_mouse_button_pressed(MouseButton::Right) {
                    actions.push(Action::Secondary(color));
                } else if is_mouse_button_pressed(MouseButton::Left) {
                    actions.push(if self.color_secondary {
                        Action::Secondary(color)
                    } else {
                        Action::Primary(color)
                    });
                }
            }
        }
        let mut color = if self.color_secondary {
            model.secondary
        } else {
            model.primary
        };
        let mut changed = false;
        for (index, name) in ["R", "G", "B", "A"].iter().enumerate() {
            let y = r.y + 167.0 + index as f32 * 18.0;
            text(name, r.x + 10.0, y + 12.0, 12.0, INK);
            if let Some(value) = slider(
                Rect::new(r.x + 27.0, y, 93.0, 15.0),
                color[index] as f32 / 255.0,
                interactive,
            ) {
                color[index] = (value * 255.0).round() as u8;
                changed = true;
            }
            text(&color[index].to_string(), r.x + 125.0, y + 12.0, 12.0, INK);
        }
        if changed {
            actions.push(if self.color_secondary {
                Action::Secondary(color)
            } else {
                Action::Primary(color)
            });
        }
        let hex = format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color[0], color[1], color[2], color[3]
        );
        if button(
            Rect::new(r.x + 9.0, r.y + 246.0, 147.0, 22.0),
            &hex,
            false,
            interactive,
        ) {
            self.set_modal(Modal::new(
                "Color",
                ModalKind::Color(self.color_secondary),
                &[("Hex RGBA", hex)],
            ));
        }
    }

    fn history_panel(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        let r = self.panel_rect(1);
        panel(r, "History");
        if close_button(r, interactive) {
            self.panels[1] = false;
        }
        let labels = model.doc.history_labels();
        let rows = ((r.h - 62.0) / 21.0).floor().max(1.0) as usize;
        self.history_scroll = self.history_scroll.min(labels.len().saturating_sub(rows));
        if interactive && r.contains(mouse()) {
            let wheel = mouse_wheel().1;
            if wheel > 0.0 {
                self.history_scroll = self.history_scroll.saturating_sub(wheel.ceil() as usize);
            } else if wheel < 0.0 {
                self.history_scroll = (self.history_scroll + (-wheel).ceil() as usize)
                    .min(labels.len().saturating_sub(rows));
            }
        }
        let position = model.doc.history_position();
        if position != self.last_history {
            if position < self.history_scroll {
                self.history_scroll = position;
            }
            if position >= self.history_scroll + rows {
                self.history_scroll = position + 1 - rows;
            }
            self.last_history = position;
        }
        for (index, label) in labels
            .iter()
            .enumerate()
            .skip(self.history_scroll)
            .take(rows)
        {
            let row = Rect::new(
                r.x + 5.0,
                r.y + 27.0 + (index - self.history_scroll) as f32 * 21.0,
                r.w - 10.0,
                20.0,
            );
            if row_button(row, label, index == position, interactive, index > position) {
                if index < position {
                    actions.extend((index..position).map(|_| Action::Undo));
                } else {
                    actions.extend((position..index).map(|_| Action::Redo));
                }
            }
        }
        if button(
            Rect::new(r.x + 7.0, r.y + r.h - 29.0, 66.0, 23.0),
            "Undo",
            false,
            interactive && model.doc.can_undo(),
        ) {
            actions.push(Action::Undo);
        }
        if button(
            Rect::new(r.x + 78.0, r.y + r.h - 29.0, 66.0, 23.0),
            "Redo",
            false,
            interactive && model.doc.can_redo(),
        ) {
            actions.push(Action::Redo);
        }
        text(
            &format!("{} / {}", position, labels.len().saturating_sub(1)),
            r.x + 151.0,
            r.y + r.h - 13.0,
            12.0,
            MUTED,
        );
    }

    fn layers_panel(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        let r = self.panel_rect(2);
        panel(r, "Layers");
        if close_button(r, interactive) {
            self.panels[2] = false;
        }
        let rows = ((r.h - 88.0) / 45.0).floor().max(1.0) as usize;
        self.layer_scroll = self
            .layer_scroll
            .min(model.doc.layers.len().saturating_sub(rows));
        if interactive && r.contains(mouse()) {
            let wheel = mouse_wheel().1;
            if wheel > 0.0 {
                self.layer_scroll = self.layer_scroll.saturating_sub(wheel.ceil() as usize);
            } else if wheel < 0.0 {
                self.layer_scroll = (self.layer_scroll + (-wheel).ceil() as usize)
                    .min(model.doc.layers.len().saturating_sub(rows));
            }
        }
        for (row_index, (index, layer)) in model
            .doc
            .layers
            .iter()
            .enumerate()
            .rev()
            .skip(self.layer_scroll)
            .take(rows)
            .enumerate()
        {
            let row = Rect::new(
                r.x + 5.0,
                r.y + 27.0 + row_index as f32 * 45.0,
                r.w - 10.0,
                43.0,
            );
            let selected = index == model.doc.active_layer;
            draw_rectangle(
                row.x,
                row.y,
                row.w,
                row.h,
                if selected { SELECTED } else { PANEL },
            );
            if selected {
                draw_rectangle_lines(row.x, row.y, row.w, row.h, 1.0, ACCENT);
            }
            let check = Rect::new(row.x + 5.0, row.y + 13.0, 15.0, 15.0);
            draw_rectangle_lines(check.x, check.y, check.w, check.h, 1.0, BORDER);
            if layer.visible {
                draw_line(
                    check.x + 3.0,
                    check.y + 8.0,
                    check.x + 6.0,
                    check.y + 11.0,
                    2.0,
                    INK,
                );
                draw_line(
                    check.x + 6.0,
                    check.y + 11.0,
                    check.x + 12.0,
                    check.y + 4.0,
                    2.0,
                    INK,
                );
            }
            let preview = Rect::new(row.x + 27.0, row.y + 7.0, 34.0, 29.0);
            checker(preview, 5.0);
            layer_preview(preview, &layer.pixels, model.doc.width, model.doc.height);
            draw_rectangle_lines(preview.x, preview.y, preview.w, preview.h, 1.0, BORDER);
            clipped_text(
                &layer.name,
                row.x + 69.0,
                row.y + 18.0,
                row.w - 75.0,
                13.0,
                INK,
            );
            text(
                &format!(
                    "{} · {}%",
                    layer.blend_mode.name(),
                    (layer.opacity as f32 / 2.55).round()
                ),
                row.x + 69.0,
                row.y + 34.0,
                10.5,
                MUTED,
            );
            if interactive && clicked(check) {
                actions.push(Action::ToggleLayer(index));
            } else if interactive && clicked(row) {
                actions.push(Action::SelectLayer(index));
            }
        }
        let bottom = r.y + r.h - 55.0;
        for (index, (label, action, enabled)) in [
            ("+", Action::AddLayer, true),
            ("−", Action::DeleteLayer, model.doc.layers.len() > 1),
            ("Dup", Action::DuplicateLayer, true),
            ("↓=", Action::MergeDown, model.doc.active_layer > 0),
            (
                "↑",
                Action::MoveLayer(1),
                model.doc.active_layer + 1 < model.doc.layers.len(),
            ),
            ("↓", Action::MoveLayer(-1), model.doc.active_layer > 0),
        ]
        .into_iter()
        .enumerate()
        {
            if button(
                Rect::new(r.x + 6.0 + index as f32 * 33.0, bottom, 30.0, 23.0),
                label,
                false,
                interactive && enabled,
            ) {
                actions.push(action);
            }
        }
        if button(
            Rect::new(r.x + 6.0, bottom + 27.0, r.w - 12.0, 22.0),
            "Layer Properties…",
            false,
            interactive,
        ) {
            self.show_layer_properties(model.doc);
        }
    }

    fn status_bar(&mut self, model: &Model<'_>, interactive: bool, actions: &mut Vec<Action>) {
        let y = screen_height() - 25.0;
        draw_rectangle(0.0, y, screen_width(), 25.0, FACE);
        draw_line(0.0, y, screen_width(), y, 1.0, BORDER);
        clipped_text(
            model.status,
            9.0,
            y + 17.0,
            (screen_width() - 390.0).max(100.0),
            12.0,
            INK,
        );
        text(
            &format!("{} × {} px", model.doc.width, model.doc.height),
            screen_width() - 355.0,
            y + 17.0,
            12.0,
            MUTED,
        );
        if button(
            Rect::new(screen_width() - 233.0, y + 2.0, 44.0, 21.0),
            "Fit",
            false,
            interactive,
        ) {
            actions.push(Action::Fit);
        }
        let value = ((model.zoom.max(0.01).log2() + 5.0) / 10.0).clamp(0.0, 1.0);
        if let Some(value) = slider(
            Rect::new(screen_width() - 181.0, y + 4.0, 105.0, 17.0),
            value,
            interactive,
        ) {
            actions.push(Action::Zoom(2.0_f32.powf(value * 10.0 - 5.0)));
        }
        text(
            &format!("{}%", (model.zoom * 100.0).round()),
            screen_width() - 65.0,
            y + 17.0,
            12.0,
            INK,
        );
    }

    fn draw_menu(&mut self, index: usize, model: &Model<'_>, actions: &mut Vec<Action>) {
        let items = self.menu_items(index, model);
        let labels = [
            "File",
            "Edit",
            "View",
            "Image",
            "Layers",
            "Adjustments",
            "Effects",
            "Window",
            "Help",
        ];
        let x = 6.0
            + labels[..index]
                .iter()
                .map(|label| menu_width(label))
                .sum::<f32>();
        let width = 285.0;
        let x = x.min((screen_width() - width - 5.0).max(0.0));
        let height: f32 = items
            .iter()
            .map(|item| {
                if matches!(item.command, MenuCommand::Separator) {
                    8.0
                } else {
                    26.0
                }
            })
            .sum();
        let r = Rect::new(x, 27.0, width, height + 8.0);
        draw_rectangle(r.x + 3.0, r.y + 3.0, r.w, r.h, color_u8!(0, 0, 0, 45));
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
        let mut y = r.y + 4.0;
        let activate = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        for (item_index, item) in items.into_iter().enumerate() {
            if matches!(item.command, MenuCommand::Separator) {
                draw_line(r.x + 8.0, y + 4.0, r.x + r.w - 8.0, y + 4.0, 1.0, BORDER);
                y += 8.0;
                continue;
            }
            let row = Rect::new(r.x + 3.0, y, r.w - 6.0, 26.0);
            if row.contains(mouse())
                && item.enabled
                && (mouse() != self.last_mouse || is_mouse_button_pressed(MouseButton::Left))
            {
                self.menu_cursor = Some(item_index);
            }
            if self.menu_cursor == Some(item_index) && item.enabled {
                draw_rectangle(row.x, row.y, row.w, row.h, SELECTED);
            }
            text(
                &item.label,
                row.x + 10.0,
                row.y + 18.0,
                14.0,
                if item.enabled { INK } else { MUTED },
            );
            let shortcut_width = measure_text(item.shortcut, None, 12, 1.0).width;
            text(
                item.shortcut,
                row.x + row.w - shortcut_width - 9.0,
                row.y + 17.0,
                12.0,
                MUTED,
            );
            if (clicked(row) || (activate && self.menu_cursor == Some(item_index))) && item.enabled
            {
                self.menu = None;
                self.menu_cursor = None;
                match item.command {
                    MenuCommand::Action(action) => actions.push(action),
                    MenuCommand::New => self.show_new(800, 600),
                    MenuCommand::Resize(canvas) => self.show_resize(model.doc.width, model.doc.height, canvas),
                    MenuCommand::Properties => self.show_layer_properties(model.doc),
                    MenuCommand::Text => self.show_text(),
                    MenuCommand::Effect(name, value, second) => {
                        if effect_labels(name).1.is_empty() { actions.push(Action::Effect { name: name.into(), value, second }); }
                        else { self.show_effect(name, value, second); }
                    }
                    MenuCommand::Panel(panel) => self.panels[panel] = !self.panels[panel],
                    MenuCommand::About => self.show_message("openpaint 0.1.0\n\na native rust image editor.\n\noriginal open-source implementation. paint.net compatibility remains incomplete; see docs/parity.md.\n\nMIT license · openpaint contributors"),
                    MenuCommand::Separator => {}
                }
            }
            y += 26.0;
        }
        if is_key_pressed(KeyCode::Escape)
            || (is_mouse_button_pressed(MouseButton::Left)
                && !r.contains(mouse())
                && mouse().y > 28.0)
        {
            self.menu = None;
        }
    }

    fn menu_items(&self, index: usize, model: &Model<'_>) -> Vec<MenuItem> {
        use MenuCommand as C;
        use MenuItem as M;
        match index {
            0 => {
                let mut rows = vec![
                    M::new("New…", "Ctrl+N", C::New),
                    M::action("Open…", "Ctrl+O", Action::Open),
                    M::separator(),
                    M::action("Save", "Ctrl+S", Action::Save),
                    M::action("Save As…", "Ctrl+Shift+S", Action::SaveAs),
                    M::separator(),
                ];
                for (index, path) in self.recent.iter().take(8).enumerate() {
                    let name = std::path::Path::new(path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy();
                    rows.push(M::action(
                        &format!("{}. {}", index + 1, name),
                        "",
                        Action::OpenRecent(index),
                    ));
                }
                if !self.recent.is_empty() {
                    rows.push(M::separator());
                }
                rows.push(M::action("Close", "Ctrl+W", Action::CloseTab(model.active)));
                rows.push(M::action("Exit", "Alt+F4", Action::Quit));
                rows
            }
            1 => vec![
                M::action("Undo", "Ctrl+Z", Action::Undo).disabled(!model.doc.can_undo()),
                M::action("Redo", "Ctrl+Y", Action::Redo).disabled(!model.doc.can_redo()),
                M::separator(),
                M::action("Cut", "Ctrl+X", Action::Cut),
                M::action("Copy", "Ctrl+C", Action::Copy),
                M::action("Paste", "Ctrl+V", Action::Paste),
                M::action(
                    "Paste into New Layer",
                    "Ctrl+Shift+V",
                    Action::PasteNewLayer,
                ),
                M::action("Paste into New Image", "Ctrl+Alt+V", Action::PasteNewImage),
                M::separator(),
                M::action("Select All", "Ctrl+A", Action::SelectAll),
                M::action("Deselect", "Ctrl+D", Action::Deselect),
                M::action("Invert Selection", "Ctrl+I", Action::InvertSelection),
                M::action("Erase Selection", "Delete", Action::DeleteSelection),
            ],
            2 => vec![
                M::action("Zoom In", "+", Action::Zoom(model.zoom * 1.25)),
                M::action("Zoom Out", "−", Action::Zoom(model.zoom / 1.25)),
                M::action("Actual Size", "Ctrl+0", Action::ActualSize),
                M::action("Zoom to Window", "Ctrl+B", Action::Fit),
            ],
            3 => vec![
                M::new("Resize…", "Ctrl+R", C::Resize(false)),
                M::new("Canvas Size…", "Ctrl+Shift+R", C::Resize(true)),
                M::action("Crop to Selection", "Ctrl+Shift+X", Action::Crop)
                    .disabled(model.doc.selection.is_none()),
                M::separator(),
                M::action("Flip Horizontal", "", Action::FlipHorizontal),
                M::action("Flip Vertical", "", Action::FlipVertical),
                M::action("Rotate 90° Clockwise", "", Action::RotateCw),
                M::action("Rotate 90° Counterclockwise", "", Action::RotateCcw),
                M::separator(),
                M::action("Flatten", "Ctrl+Shift+F", Action::Flatten),
            ],
            4 => vec![
                M::action("Add New Layer", "Ctrl+Shift+N", Action::AddLayer),
                M::action("Delete Layer", "", Action::DeleteLayer)
                    .disabled(model.doc.layers.len() == 1),
                M::action("Duplicate Layer", "Ctrl+Shift+D", Action::DuplicateLayer),
                M::action("Merge Layer Down", "Ctrl+M", Action::MergeDown)
                    .disabled(model.doc.active_layer == 0),
                M::separator(),
                M::action("Move Layer Up", "", Action::MoveLayer(1))
                    .disabled(model.doc.active_layer + 1 == model.doc.layers.len()),
                M::action("Move Layer Down", "", Action::MoveLayer(-1))
                    .disabled(model.doc.active_layer == 0),
                M::action(
                    "Toggle Layer Visibility",
                    "Ctrl+,",
                    Action::ToggleLayer(model.doc.active_layer),
                ),
                M::separator(),
                M::new("Layer Properties…", "F4", C::Properties),
            ],
            5 => vec![
                M::new("Auto-Level", "", C::Effect("auto_levels", 0.0, 0.0)),
                M::new("Black and White", "", C::Effect("grayscale", 0.0, 0.0)),
                M::new(
                    "Brightness / Contrast…",
                    "",
                    C::Effect("brightness_contrast", 0.0, 0.0),
                ),
                M::new(
                    "Hue / Saturation…",
                    "",
                    C::Effect("hue_saturation", 0.0, 100.0),
                ),
                M::new("Invert Colors", "", C::Effect("invert", 0.0, 0.0)),
                M::new("Invert Alpha", "", C::Effect("invert_alpha", 0.0, 0.0)),
                M::new("Exposure…", "", C::Effect("exposure", 0.0, 0.0)),
                M::new(
                    "Temperature / Tint…",
                    "",
                    C::Effect("temperature_tint", 0.0, 0.0),
                ),
                M::new("Posterize…", "", C::Effect("posterize", 6.0, 0.0)),
                M::new("Sepia", "", C::Effect("sepia", 0.0, 0.0)),
            ],
            6 => vec![
                M::new("Gaussian Blur…", "", C::Effect("gaussian_blur", 2.0, 0.0)),
                M::new("Motion Blur…", "", C::Effect("motion_blur", 10.0, 0.0)),
                M::new("Zoom Blur…", "", C::Effect("zoom_blur", 20.0, 0.0)),
                M::new("Radial Blur…", "", C::Effect("radial_blur", 10.0, 0.0)),
                M::new("Median…", "", C::Effect("median", 1.0, 0.0)),
                M::separator(),
                M::new("Add Noise…", "", C::Effect("noise", 20.0, 0.0)),
                M::new("Pixelate…", "", C::Effect("pixelate", 8.0, 0.0)),
                M::new("Frosted Glass…", "", C::Effect("frosted_glass", 5.0, 0.0)),
                M::new("Twist…", "", C::Effect("twist", 45.0, 0.0)),
                M::new("Bulge…", "", C::Effect("bulge", 25.0, 0.0)),
                M::separator(),
                M::new("Glow…", "", C::Effect("glow", 5.0, 100.0)),
                M::new("Sharpen…", "", C::Effect("sharpen", 1.0, 0.0)),
                M::new("Vignette…", "", C::Effect("vignette", 50.0, 0.0)),
                M::new("Pencil Sketch…", "", C::Effect("pencil_sketch", 5.0, 0.0)),
                M::new("Ink Sketch…", "", C::Effect("ink_sketch", 5.0, 0.0)),
                M::new("Edge Detect", "", C::Effect("edge_detect", 0.0, 0.0)),
                M::new("Emboss", "", C::Effect("emboss", 0.0, 0.0)),
            ],
            7 => ["Tools", "History", "Layers", "Colors"]
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    M::new(
                        &format!("{} {}", if self.panels[index] { "✓" } else { "  " }, name),
                        ["F5", "F6", "F7", "F8"][index],
                        C::Panel(index),
                    )
                })
                .collect(),
            _ => vec![
                M::new("Enter Text…", "", C::Text),
                M::new("About openpaint", "", C::About),
            ],
        }
    }

    fn draw_modal(&mut self, actions: &mut Vec<Action>, busy: bool) {
        let Some(mut modal) = self.modal.take() else {
            return;
        };
        let field_count = modal.fields.len();
        if !busy && field_count > 0 && is_key_pressed(KeyCode::Tab) {
            let direction = if is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift) {
                field_count - 1
            } else {
                1
            };
            self.focus = Some((self.focus.unwrap_or(0) + direction) % field_count);
            if let Some(index) = self.focus {
                let field = &mut modal.fields[index];
                field.edit.select_all(&field.value);
            }
        }
        draw_rectangle(
            0.0,
            0.0,
            screen_width(),
            screen_height(),
            color_u8!(0, 0, 0, 65),
        );
        let message_lines = wrap_lines(&modal.message, 450.0, 14.0);
        let message_height = if modal.message.is_empty() {
            0.0
        } else {
            message_lines.len() as f32 * 19.0 + 12.0
        };
        let extra = if matches!(modal.kind, ModalKind::Layer(_)) {
            39.0
        } else {
            0.0
        };
        let multiline = matches!(modal.kind, ModalKind::Text);
        let height = 110.0
            + field_count as f32 * 51.0
            + message_height
            + extra
            + if multiline { 68.0 } else { 0.0 };
        let r = Rect::new(
            (screen_width() - 490.0) / 2.0,
            (screen_height() - height) / 2.0,
            490.0,
            height,
        );
        draw_rectangle(r.x + 5.0, r.y + 5.0, r.w, r.h, color_u8!(0, 0, 0, 50));
        panel(r, &modal.title);
        let mut y = r.y + 43.0;
        for line in &message_lines {
            text(line, r.x + 18.0, y, 14.0, INK);
            y += 19.0;
        }
        if !modal.message.is_empty() {
            y += 12.0;
        }
        if !is_mouse_button_down(MouseButton::Left) {
            self.text_drag = None;
        }
        for (index, field) in modal.fields.iter_mut().enumerate() {
            let multiline_field = multiline && index == 0;
            text(&field.label, r.x + 19.0, y, 13.0, INK);
            if multiline_field {
                text("Shift+Enter: new line", r.x + 270.0, y, 12.0, MUTED);
            }
            let input = Rect::new(
                r.x + 18.0,
                y + 8.0,
                r.w - 36.0,
                if multiline_field { 96.0 } else { 28.0 },
            );
            if !busy && clicked(input) {
                self.focus = Some(index);
                self.text_drag = Some(index);
                let cursor = text_cursor_at(field, input, mouse());
                field.edit.move_to(cursor, shift_down());
            }
            if !busy && self.text_drag == Some(index) && is_mouse_button_down(MouseButton::Left) {
                let cursor = text_cursor_at(field, input, mouse());
                field.edit.move_to(cursor, true);
            }
            if !busy && self.focus == Some(index) {
                edit_text(&mut field.value, &mut field.edit, multiline_field);
            }
            draw_text_field(field, input, self.focus == Some(index));
            y += input.h + 23.0;
        }
        if let ModalKind::Layer(ref mut mode) = modal.kind {
            text("Blending mode", r.x + 19.0, y + 6.0, 13.0, INK);
            if button(
                Rect::new(r.x + 150.0, y - 11.0, 28.0, 27.0),
                "‹",
                false,
                true,
            ) {
                *mode = (*mode + BlendMode::ALL.len() - 1) % BlendMode::ALL.len();
            }
            text(
                BlendMode::ALL[*mode].name(),
                r.x + 191.0,
                y + 7.0,
                14.0,
                INK,
            );
            if button(
                Rect::new(r.x + r.w - 48.0, y - 11.0, 28.0, 27.0),
                "›",
                false,
                true,
            ) {
                *mode = (*mode + 1) % BlendMode::ALL.len();
            }
        }
        clipped_text(
            &modal.error,
            r.x + 18.0,
            r.y + r.h - 49.0,
            r.w - 36.0,
            12.0,
            ERROR,
        );
        let button_y = r.y + r.h - 37.0;
        let enter = !busy
            && !(multiline && self.focus == Some(0) && shift_down())
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter));
        let can_cancel = !busy || matches!(modal.kind, ModalKind::Effect(_));
        let escape = can_cancel && is_key_pressed(KeyCode::Escape);
        if matches!(modal.kind, ModalKind::Confirm) {
            if button(
                Rect::new(r.x + 151.0, button_y, 90.0, 27.0),
                "Save",
                true,
                true,
            ) || enter
            {
                actions.push(Action::ConfirmSave);
                self.focus = None;
                return;
            }
            if button(
                Rect::new(r.x + 246.0, button_y, 125.0, 27.0),
                "Don't Save",
                false,
                true,
            ) {
                actions.push(Action::ConfirmDiscard);
                self.focus = None;
                return;
            }
            if button(
                Rect::new(r.x + 377.0, button_y, 94.0, 27.0),
                "Cancel",
                false,
                true,
            ) || escape
            {
                actions.push(Action::ConfirmCancel);
                self.focus = None;
                return;
            }
        } else if matches!(modal.kind, ModalKind::Message) {
            if button(
                Rect::new(r.x + r.w - 110.0, button_y, 91.0, 27.0),
                "OK",
                true,
                true,
            ) || enter
                || escape
            {
                self.focus = None;
                return;
            }
        } else {
            if matches!(modal.kind, ModalKind::Effect(_))
                && button(
                    Rect::new(r.x + 18.0, button_y, 90.0, 27.0),
                    "Preview",
                    false,
                    !busy,
                )
            {
                match modal_action(&modal) {
                    Ok(Action::Effect {
                        name,
                        value,
                        second,
                    }) => actions.push(Action::PreviewEffect {
                        name,
                        value,
                        second,
                    }),
                    Err(error) => modal.error = error,
                    _ => {}
                }
            }
            let label = match modal.kind {
                ModalKind::Path(true) => "Save",
                ModalKind::Path(false) => "Open",
                ModalKind::Effect(_) => "Apply",
                _ => "OK",
            };
            if button(
                Rect::new(r.x + r.w - 207.0, button_y, 90.0, 27.0),
                label,
                true,
                !busy,
            ) || enter
            {
                match modal_action(&modal) {
                    Ok(action) => {
                        actions.push(action);
                        self.focus = None;
                        return;
                    }
                    Err(error) => modal.error = error,
                }
            }
            if button(
                Rect::new(r.x + r.w - 109.0, button_y, 90.0, 27.0),
                "Cancel",
                false,
                can_cancel,
            ) || escape
            {
                if matches!(modal.kind, ModalKind::Effect(_)) {
                    actions.push(Action::CancelEffect);
                } else if matches!(modal.kind, ModalKind::Path(_)) {
                    actions.push(Action::ConfirmCancel);
                }
                self.focus = None;
                return;
            }
        }
        self.modal = Some(modal);
    }
}

fn modal_action(modal: &Modal) -> Result<Action, String> {
    let value = |index: usize| {
        modal
            .fields
            .get(index)
            .map(|f| f.value.trim())
            .unwrap_or("")
    };
    let number = |index: usize| -> Result<f32, String> {
        let number: f32 = value(index).parse().map_err(|_| "enter a valid number")?;
        if !number.is_finite() {
            return Err("enter a finite number".into());
        }
        Ok(number)
    };
    let dimensions = || -> Result<(u32, u32), String> {
        let width: u32 = value(0)
            .parse()
            .map_err(|_| "width must be a whole number")?;
        let height: u32 = value(1)
            .parse()
            .map_err(|_| "height must be a whole number")?;
        openpaint::document::validate_dimensions(width, height)?;
        Ok((width, height))
    };
    match &modal.kind {
        ModalKind::New => {
            let (width, height) = dimensions()?;
            Ok(Action::New { width, height })
        }
        ModalKind::Resize(canvas) => {
            let (width, height) = dimensions()?;
            Ok(Action::Resize {
                width,
                height,
                canvas: *canvas,
            })
        }
        ModalKind::Layer(mode) => {
            let opacity: u8 = value(1)
                .parse()
                .map_err(|_| "opacity must be between 0 and 255")?;
            if value(0).is_empty() {
                return Err("enter a layer name".into());
            }
            Ok(Action::LayerProperties {
                name: value(0).into(),
                opacity,
                blend: BlendMode::ALL[*mode],
            })
        }
        ModalKind::Text => {
            let size = number(1)?;
            if !(1.0..=512.0).contains(&size) {
                return Err("text size must be between 1 and 512".into());
            }
            if value(0).is_empty() {
                return Err("enter some text".into());
            }
            Ok(Action::Text {
                text: modal.fields[0].value.clone(),
                size,
            })
        }
        ModalKind::Effect(name) => Ok(Action::Effect {
            name: name.clone(),
            value: if modal.fields.is_empty() {
                0.0
            } else {
                number(0)?
            },
            second: if modal.fields.len() > 1 {
                number(1)?
            } else {
                0.0
            },
        }),
        ModalKind::Path(save) => {
            if value(0).is_empty() {
                return Err("enter a file path".into());
            }
            Ok(if *save {
                Action::SavePath(value(0).into())
            } else {
                Action::OpenPath(value(0).into())
            })
        }
        ModalKind::Color(secondary) => {
            let hex = value(0).trim_start_matches('#');
            if hex.len() != 6 && hex.len() != 8 {
                return Err("use six RGB or eight RGBA hexadecimal digits".into());
            }
            let encoded = u32::from_str_radix(hex, 16).map_err(|_| "invalid hexadecimal color")?;
            let color = if hex.len() == 8 {
                encoded.to_be_bytes()
            } else {
                [
                    (encoded >> 16) as u8,
                    (encoded >> 8) as u8,
                    encoded as u8,
                    255,
                ]
            };
            Ok(if *secondary {
                Action::Secondary(color)
            } else {
                Action::Primary(color)
            })
        }
        ModalKind::Confirm | ModalKind::Message => Err("invalid dialog action".into()),
    }
}

fn effect_labels(name: &str) -> (&'static str, &'static str, &'static str) {
    match name {
        "brightness_contrast" => ("Brightness / Contrast", "Brightness", "Contrast"),
        "hue_saturation" => (
            "Hue / Saturation",
            "Hue (degrees)",
            "Saturation (0–200%, 100 = original)",
        ),
        "posterize" => ("Posterize", "Levels (2–64)", ""),
        "gaussian_blur" => ("Gaussian Blur", "Radius (pixels)", ""),
        "motion_blur" => ("Motion Blur", "Radius (pixels)", "Angle (degrees)"),
        "median" => ("Median", "Radius (pixels)", ""),
        "pixelate" => ("Pixelate", "Cell size (pixels)", ""),
        "noise" => ("Add Noise", "Amount (0–100)", ""),
        "glow" => ("Glow", "Radius (pixels)", "Amount (0–200%)"),
        "sharpen" => ("Sharpen", "Amount", ""),
        "invert" => ("Invert Colors", "", ""),
        "grayscale" => ("Black and White", "", ""),
        "sepia" => ("Sepia", "", ""),
        "auto_levels" => ("Auto-Level", "", ""),
        "emboss" => ("Emboss", "", ""),
        "invert_alpha" => ("Invert Alpha", "", ""),
        "exposure" => ("Exposure", "Stops (−5 to 5)", ""),
        "temperature_tint" => (
            "Temperature / Tint",
            "Temperature (−100 to 100)",
            "Tint (−100 to 100)",
        ),
        "vignette" => ("Vignette", "Amount (0–100)", ""),
        "frosted_glass" => ("Frosted Glass", "Radius (0–50 pixels)", ""),
        "twist" => ("Twist", "Angle (−180 to 180 degrees)", ""),
        "bulge" => ("Bulge", "Amount (−100 to 100)", ""),
        "zoom_blur" => ("Zoom Blur", "Amount (0–100)", ""),
        "radial_blur" => ("Radial Blur", "Angle (0–180 degrees)", ""),
        "pencil_sketch" => ("Pencil Sketch", "Radius (1–20 pixels)", ""),
        "ink_sketch" => ("Ink Sketch", "Radius (1–20 pixels)", ""),
        _ => ("Edge Detect", "", ""),
    }
}

fn mouse() -> Vec2 {
    let (x, y) = mouse_position();
    vec2(x, y)
}

fn next_menu_item(items: &[usize], current: Option<usize>, backwards: bool) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let position = current.and_then(|current| items.iter().position(|&item| item == current));
    let next = match position {
        Some(index) if backwards => (index + items.len() - 1) % items.len(),
        Some(index) => (index + 1) % items.len(),
        None if backwards => items.len() - 1,
        None => 0,
    };
    Some(items[next])
}
fn clicked(r: Rect) -> bool {
    r.contains(mouse()) && is_mouse_button_pressed(MouseButton::Left)
}
fn menu_width(label: &str) -> f32 {
    measure_text(label, None, 14, 1.0).width + 18.0
}
fn text(label: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text(label, x, y, size, color);
}

fn button(r: Rect, label: &str, selected: bool, enabled: bool) -> bool {
    let hover = enabled && r.contains(mouse());
    let fill = if selected {
        SELECTED
    } else if hover {
        HOVER
    } else {
        FACE
    };
    draw_rectangle(r.x, r.y, r.w, r.h, fill);
    if hover || selected {
        draw_rectangle_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            1.0,
            if selected { ACCENT } else { BORDER },
        );
    }
    let size = if r.w < 35.0 { 16.0 } else { 13.0 };
    let width = measure_text(label, None, size as u16, 1.0).width;
    text(
        label,
        r.x + (r.w - width) / 2.0,
        r.y + r.h / 2.0 + size / 3.0,
        size,
        if enabled { INK } else { MUTED },
    );
    enabled && clicked(r)
}

fn row_button(r: Rect, label: &str, selected: bool, enabled: bool, dim: bool) -> bool {
    if selected || (enabled && r.contains(mouse())) {
        draw_rectangle(r.x, r.y, r.w, r.h, if selected { SELECTED } else { HOVER });
    }
    clipped_text(
        label,
        r.x + 7.0,
        r.y + 14.0,
        r.w - 12.0,
        12.0,
        if dim { MUTED } else { INK },
    );
    enabled && clicked(r)
}

fn panel(r: Rect, title: &str) {
    draw_rectangle(r.x + 2.0, r.y + 2.0, r.w, r.h, color_u8!(0, 0, 0, 25));
    draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
    draw_rectangle(r.x, r.y, r.w, 23.0, FACE);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, BORDER);
    if !title.is_empty() {
        text(title, r.x + 7.0, r.y + 16.0, 13.0, INK);
    }
}

fn close_button(r: Rect, enabled: bool) -> bool {
    button(
        Rect::new(r.x + r.w - 22.0, r.y + 2.0, 19.0, 19.0),
        "×",
        false,
        enabled,
    )
}

fn slider(r: Rect, value: f32, enabled: bool) -> Option<f32> {
    draw_rectangle(r.x, r.y + r.h / 2.0 - 2.0, r.w, 4.0, BORDER);
    let x = r.x + value.clamp(0.0, 1.0) * r.w;
    draw_rectangle(
        x - 3.0,
        r.y + 1.0,
        6.0,
        r.h - 2.0,
        if enabled { ACCENT } else { MUTED },
    );
    if enabled && r.contains(mouse()) && is_mouse_button_down(MouseButton::Left) {
        Some(((mouse().x - r.x) / r.w).clamp(0.0, 1.0))
    } else {
        None
    }
}

fn checker(r: Rect, size: f32) {
    draw_rectangle(r.x, r.y, r.w, r.h, CHECKER_LIGHT);
    let columns = (r.w / size).ceil() as usize;
    let rows = (r.h / size).ceil() as usize;
    for y in 0..rows {
        for x in 0..columns {
            if (x + y) % 2 == 0 {
                draw_rectangle(
                    r.x + x as f32 * size,
                    r.y + y as f32 * size,
                    size.min(r.w - x as f32 * size),
                    size.min(r.h - y as f32 * size),
                    CHECKER_DARK,
                );
            }
        }
    }
}

fn swatch(r: Rect, pixel: Pixel, selected: bool) {
    checker(r, 6.0);
    draw_rectangle(
        r.x,
        r.y,
        r.w,
        r.h,
        Color::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3]),
    );
    draw_rectangle_lines(
        r.x,
        r.y,
        r.w,
        r.h,
        if selected { 2.0 } else { 1.0 },
        if selected { ACCENT } else { BORDER },
    );
}

fn clipped_text(label: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    if measure_text(label, None, size as u16, 1.0).width <= width {
        text(label, x, y, size, color);
        return;
    }
    let mut short = String::new();
    for character in label.chars() {
        let mut next = short.clone();
        next.push(character);
        next.push('…');
        if measure_text(&next, None, size as u16, 1.0).width > width {
            break;
        }
        short.push(character);
    }
    short.push('…');
    text(&short, x, y, size, color);
}

fn wrap_lines(value: &str, width: f32, size: f32) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for paragraph in value.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure_text(&candidate, None, size as u16, 1.0).width > width {
                lines.push(line);
                line = word.into();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

fn shift_down() -> bool {
    is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift)
}

impl TextEdit {
    fn select_all(&mut self, value: &str) {
        self.anchor = 0;
        self.cursor = value.len();
    }
    fn range(&self) -> std::ops::Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }
    fn move_to(&mut self, cursor: usize, extend: bool) {
        self.cursor = cursor;
        if !extend {
            self.anchor = cursor;
        }
    }
    fn replace(&mut self, value: &mut String, inserted: &str) {
        let range = self.range();
        if value.len() - range.len() + inserted.len() > 8192 {
            return;
        }
        value.replace_range(range.clone(), inserted);
        self.move_to(range.start + inserted.len(), false);
    }
    fn backspace(&mut self, value: &mut String) {
        if self.range().is_empty() {
            self.anchor = previous_char(value, self.cursor);
        }
        self.replace(value, "");
    }
    fn delete(&mut self, value: &mut String) {
        if self.range().is_empty() {
            self.anchor = next_char(value, self.cursor);
        }
        self.replace(value, "");
    }
    fn horizontal(&mut self, value: &str, backwards: bool, extend: bool) {
        let range = self.range();
        let cursor = if !extend && !range.is_empty() {
            if backwards {
                range.start
            } else {
                range.end
            }
        } else if backwards {
            previous_char(value, self.cursor)
        } else {
            next_char(value, self.cursor)
        };
        self.move_to(cursor, extend);
    }
    fn home_end(&mut self, value: &str, end: bool, document: bool, extend: bool) {
        let line = line_bounds(value, self.cursor);
        self.move_to(
            if document {
                if end {
                    value.len()
                } else {
                    0
                }
            } else if end {
                line.end
            } else {
                line.start
            },
            extend,
        );
    }
    fn vertical(&mut self, value: &str, backwards: bool, extend: bool) {
        let line = line_bounds(value, self.cursor);
        let column = value[line.start..self.cursor].chars().count();
        let destination = if backwards {
            if line.start == 0 {
                self.move_to(0, extend);
                return;
            }
            line_bounds(value, line.start - 1)
        } else {
            if line.end == value.len() {
                self.move_to(value.len(), extend);
                return;
            }
            line_bounds(value, line.end + 1)
        };
        let cursor = value[destination.clone()]
            .char_indices()
            .nth(column)
            .map_or(destination.end, |(offset, _)| destination.start + offset);
        self.move_to(cursor, extend);
    }
}

fn previous_char(value: &str, cursor: usize) -> usize {
    value[..cursor]
        .char_indices()
        .next_back()
        .map_or(0, |(offset, _)| offset)
}
fn next_char(value: &str, cursor: usize) -> usize {
    value[cursor..]
        .chars()
        .next()
        .map_or(value.len(), |character| cursor + character.len_utf8())
}
fn line_bounds(value: &str, cursor: usize) -> std::ops::Range<usize> {
    let start = value[..cursor].rfind('\n').map_or(0, |offset| offset + 1);
    let end = value[cursor..]
        .find('\n')
        .map_or(value.len(), |offset| cursor + offset);
    start..end
}
fn text_lines(value: &str) -> Vec<std::ops::Range<usize>> {
    let mut start = 0;
    value
        .split('\n')
        .map(|line| {
            let range = start..start + line.len();
            start += line.len() + 1;
            range
        })
        .collect()
}

fn edit_text(value: &mut String, edit: &mut TextEdit, multiline: bool) {
    let command = is_key_down(KeyCode::LeftControl)
        || is_key_down(KeyCode::RightControl)
        || is_key_down(KeyCode::LeftSuper)
        || is_key_down(KeyCode::RightSuper);
    let shift = shift_down();
    if command && is_key_pressed(KeyCode::A) {
        edit.select_all(value);
    }
    if command && is_key_pressed(KeyCode::C) && !edit.range().is_empty() {
        macroquad::miniquad::window::clipboard_set(&value[edit.range()]);
    }
    if command && is_key_pressed(KeyCode::X) && !edit.range().is_empty() {
        macroquad::miniquad::window::clipboard_set(&value[edit.range()]);
        edit.replace(value, "");
    }
    if command && is_key_pressed(KeyCode::V) {
        if let Some(paste) = macroquad::miniquad::window::clipboard_get() {
            let paste = paste.replace("\r\n", "\n").replace('\r', "\n");
            let paste = if multiline {
                paste
            } else {
                paste.replace('\n', " ")
            };
            edit.replace(value, &paste);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        edit.backspace(value);
    }
    if is_key_pressed(KeyCode::Delete) {
        edit.delete(value);
    }
    if is_key_pressed(KeyCode::Left) {
        edit.horizontal(value, true, shift);
    }
    if is_key_pressed(KeyCode::Right) {
        edit.horizontal(value, false, shift);
    }
    if is_key_pressed(KeyCode::Home) {
        edit.home_end(value, false, command, shift);
    }
    if is_key_pressed(KeyCode::End) {
        edit.home_end(value, true, command, shift);
    }
    if multiline && is_key_pressed(KeyCode::Up) {
        edit.vertical(value, true, shift);
    }
    if multiline && is_key_pressed(KeyCode::Down) {
        edit.vertical(value, false, shift);
    }
    if multiline && shift && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter)) {
        edit.replace(value, "\n");
    }
    while let Some(character) = get_char_pressed() {
        if !command && !character.is_control() {
            let mut bytes = [0; 4];
            edit.replace(value, character.encode_utf8(&mut bytes));
        }
    }
}

fn text_cursor_at(field: &Field, input: Rect, pointer: Vec2) -> usize {
    let lines = text_lines(&field.value);
    let row = (((pointer.y - input.y - 6.0) / 20.0).floor().max(0.0) as usize
        + field.edit.first_line)
        .min(lines.len() - 1);
    let range = &lines[row];
    let line = &field.value[range.clone()];
    let x = pointer.x - input.x - 7.0 + field.edit.scroll_x;
    let mut previous = 0.0;
    for (offset, character) in line.char_indices() {
        let next = measure_text(&line[..offset + character.len_utf8()], None, 14, 1.0).width;
        if x < (previous + next) / 2.0 {
            return range.start + offset;
        }
        previous = next;
    }
    range.end
}

fn draw_text_field(field: &mut Field, input: Rect, focused: bool) {
    draw_rectangle(input.x, input.y, input.w, input.h, FIELD);
    draw_rectangle_lines(
        input.x,
        input.y,
        input.w,
        input.h,
        if focused { 2.0 } else { 1.0 },
        if focused { ACCENT } else { BORDER },
    );
    let lines = text_lines(&field.value);
    let caret_row = lines
        .iter()
        .position(|range| range.contains(&field.edit.cursor) || range.end == field.edit.cursor)
        .unwrap_or(lines.len() - 1);
    let visible_rows = ((input.h - 10.0) / 20.0).floor().max(1.0) as usize;
    if caret_row < field.edit.first_line {
        field.edit.first_line = caret_row;
    }
    if caret_row >= field.edit.first_line + visible_rows {
        field.edit.first_line = caret_row + 1 - visible_rows;
    }
    let caret_x = measure_text(
        &field.value[lines[caret_row].start..field.edit.cursor],
        None,
        14,
        1.0,
    )
    .width;
    if caret_x < field.edit.scroll_x {
        field.edit.scroll_x = caret_x;
    }
    if caret_x > field.edit.scroll_x + input.w - 18.0 {
        field.edit.scroll_x = caret_x - input.w + 18.0;
    }
    for (row, range) in lines
        .iter()
        .enumerate()
        .skip(field.edit.first_line)
        .take(visible_rows)
    {
        let y = input.y + 6.0 + (row - field.edit.first_line) as f32 * 20.0;
        let line = &field.value[range.clone()];
        let selection = field.edit.range();
        if focused && selection.start < range.end && selection.end > range.start {
            let start = selection.start.max(range.start) - range.start;
            let end = selection.end.min(range.end) - range.start;
            let x1 = (measure_text(&line[..start], None, 14, 1.0).width - field.edit.scroll_x)
                .clamp(0.0, input.w - 14.0);
            let x2 = (measure_text(&line[..end], None, 14, 1.0).width - field.edit.scroll_x)
                .clamp(0.0, input.w - 14.0);
            draw_rectangle(input.x + 7.0 + x1, y, x2 - x1, 18.0, SELECTED);
        }
        let mut start = 0;
        let mut end = line.len();
        let mut left = 0.0;
        for (offset, _) in line.char_indices() {
            let width = measure_text(&line[..offset], None, 14, 1.0).width;
            if width < field.edit.scroll_x {
                start = offset;
                left = width;
            }
            if width - field.edit.scroll_x > input.w - 18.0 {
                end = offset;
                break;
            }
        }
        text(
            &line[start..end],
            input.x + 7.0 + (left - field.edit.scroll_x).max(0.0),
            y + 14.0,
            14.0,
            INK,
        );
        if focused && row == caret_row && (get_time() * 2.0) as i64 % 2 == 0 {
            let x = input.x + 7.0 + caret_x - field.edit.scroll_x;
            draw_line(x, y, x, y + 17.0, 1.0, INK);
        }
    }
}

fn layer_preview(r: Rect, pixels: &[u8], width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    let columns = 12;
    let rows = 10;
    for y in 0..rows {
        for x in 0..columns {
            let px = x * width as usize / columns;
            let py = y * height as usize / rows;
            let offset = (py * width as usize + px) * 4;
            if let Some(pixel) = pixels.get(offset..offset + 4) {
                draw_rectangle(
                    r.x + x as f32 * r.w / columns as f32,
                    r.y + y as f32 * r.h / rows as f32,
                    r.w / columns as f32 + 0.2,
                    r.h / rows as f32 + 0.2,
                    Color::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3]),
                );
            }
        }
    }
}

fn icon_button(r: Rect, tool: Tool, selected: bool, enabled: bool) -> bool {
    if selected || (enabled && r.contains(mouse())) {
        draw_rectangle(r.x, r.y, r.w, r.h, if selected { SELECTED } else { HOVER });
        draw_rectangle_lines(
            r.x,
            r.y,
            r.w,
            r.h,
            1.0,
            if selected { ACCENT } else { BORDER },
        );
    }
    let x = r.x + r.w / 2.0;
    let y = r.y + r.h / 2.0;
    let ink = if enabled { INK } else { MUTED };
    match tool {
        Tool::RectangleSelect => dashed_box(Rect::new(x - 9.0, y - 7.0, 18.0, 14.0), ink),
        Tool::EllipseSelect => {
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::TAU / 12.0;
                let b = a + 0.27;
                draw_line(
                    x + a.cos() * 9.0,
                    y + a.sin() * 7.0,
                    x + b.cos() * 9.0,
                    y + b.sin() * 7.0,
                    1.4,
                    ink,
                );
            }
        }
        Tool::Lasso => {
            draw_ellipse_lines(x, y, 9.0, 6.0, 0.0, 1.5, ink);
            draw_line(x - 5.0, y + 5.0, x - 9.0, y + 10.0, 1.5, ink);
        }
        Tool::MovePixels | Tool::MoveSelection | Tool::Pan => {
            draw_line(x - 9.0, y, x + 9.0, y, 1.5, ink);
            draw_line(x, y - 9.0, x, y + 9.0, 1.5, ink);
            for (dx, dy) in [(-9.0, 0.0), (9.0, 0.0), (0.0, -9.0), (0.0, 9.0)] {
                draw_triangle(
                    vec2(x + dx, y + dy),
                    vec2(x + dx * 0.5 - dy * 0.3, y + dy * 0.5 + dx * 0.3),
                    vec2(x + dx * 0.5 + dy * 0.3, y + dy * 0.5 - dx * 0.3),
                    if tool == Tool::MoveSelection {
                        MUTED
                    } else {
                        ACCENT
                    },
                );
            }
        }
        Tool::Zoom => {
            draw_circle_lines(x - 2.0, y - 2.0, 6.0, 1.8, ink);
            draw_line(x + 3.0, y + 3.0, x + 10.0, y + 10.0, 3.0, ink);
        }
        Tool::MagicWand => {
            draw_line(x - 7.0, y + 8.0, x + 5.0, y - 5.0, 3.0, ink);
            for a in 0..4 {
                let angle = a as f32 * std::f32::consts::FRAC_PI_2;
                draw_line(
                    x + 6.0 + angle.cos() * 4.0,
                    y - 7.0 + angle.sin() * 4.0,
                    x + 6.0 + angle.cos() * 7.0,
                    y - 7.0 + angle.sin() * 7.0,
                    1.5,
                    ACCENT,
                );
            }
        }
        Tool::Paintbrush | Tool::Pencil | Tool::ColorPicker | Tool::Recolor => {
            draw_line(
                x - 5.0,
                y + 6.0,
                x + 7.0,
                y - 8.0,
                if tool == Tool::Pencil { 3.0 } else { 5.0 },
                if tool == Tool::Pencil {
                    color_u8!(177, 129, 35, 255)
                } else {
                    ACCENT
                },
            );
            draw_triangle(
                vec2(x - 9.0, y + 11.0),
                vec2(x - 7.0, y + 3.0),
                vec2(x - 2.0, y + 7.0),
                ink,
            );
            if tool == Tool::Recolor {
                draw_circle(x + 8.0, y + 8.0, 4.0, color_u8!(186, 74, 56, 255));
            }
        }
        Tool::Eraser => {
            draw_rectangle_ex(
                x - 8.0,
                y - 4.0,
                17.0,
                11.0,
                DrawRectangleParams {
                    rotation: -0.5,
                    offset: vec2(0.0, 0.0),
                    color: color_u8!(206, 127, 139, 255),
                },
            );
            draw_line(x - 10.0, y + 10.0, x + 10.0, y + 10.0, 1.0, ink);
        }
        Tool::PaintBucket => {
            draw_rectangle_ex(
                x - 7.0,
                y - 9.0,
                13.0,
                13.0,
                DrawRectangleParams {
                    rotation: 0.6,
                    offset: vec2(0.0, 0.0),
                    color: ACCENT,
                },
            );
            draw_triangle(
                vec2(x + 7.0, y + 1.0),
                vec2(x + 4.0, y + 9.0),
                vec2(x + 10.0, y + 9.0),
                ACCENT,
            );
        }
        Tool::Gradient => {
            for i in 0..6 {
                let n = 30 + i * 38;
                draw_rectangle(
                    x - 10.0 + i as f32 * 3.5,
                    y - 7.0,
                    3.5,
                    14.0,
                    Color::from_rgba(n as u8, n as u8, n as u8, 255),
                );
            }
            draw_rectangle_lines(x - 10.0, y - 7.0, 21.0, 14.0, 1.0, ink);
        }
        Tool::CloneStamp => {
            draw_rectangle(x - 8.0, y + 4.0, 16.0, 5.0, ink);
            draw_rectangle(x - 4.0, y - 3.0, 8.0, 7.0, ink);
            draw_circle(x, y - 6.0, 5.0, ACCENT);
        }
        Tool::Text => text("T", x - 7.0, y + 8.0, 26.0, ink),
        Tool::Line => {
            draw_line(x - 9.0, y + 8.0, x + 9.0, y - 8.0, 2.0, ACCENT);
            draw_rectangle(x - 11.0, y + 6.0, 4.0, 4.0, ink);
            draw_rectangle(x + 7.0, y - 10.0, 4.0, 4.0, ink);
        }
        Tool::Rectangle => draw_rectangle_lines(x - 9.0, y - 7.0, 18.0, 14.0, 2.0, ACCENT),
        Tool::Ellipse => draw_ellipse_lines(x, y, 10.0, 7.0, 0.0, 2.0, ACCENT),
        Tool::RoundedRectangle => {
            draw_rectangle_lines(x - 7.0, y - 7.0, 14.0, 14.0, 2.0, ACCENT);
            draw_circle_lines(x - 5.0, y - 5.0, 3.0, 1.0, ACCENT);
        }
    }
    enabled && clicked(r)
}

fn dashed_box(r: Rect, color: Color) {
    for x in (0..r.w as i32).step_by(5) {
        draw_line(
            r.x + x as f32,
            r.y,
            (r.x + x as f32 + 3.0).min(r.x + r.w),
            r.y,
            1.0,
            color,
        );
        draw_line(
            r.x + x as f32,
            r.y + r.h,
            (r.x + x as f32 + 3.0).min(r.x + r.w),
            r.y + r.h,
            1.0,
            color,
        );
    }
    for y in (0..r.h as i32).step_by(5) {
        draw_line(
            r.x,
            r.y + y as f32,
            r.x,
            (r.y + y as f32 + 3.0).min(r.y + r.h),
            1.0,
            color,
        );
        draw_line(
            r.x + r.w,
            r.y + y as f32,
            r.x + r.w,
            (r.y + y as f32 + 3.0).min(r.y + r.h),
            1.0,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_preserves_unicode_boundaries() {
        let mut value = "aé🎨z".to_string();
        let mut edit = TextEdit {
            cursor: "aé🎨".len(),
            anchor: "aé🎨".len(),
            ..TextEdit::default()
        };
        edit.backspace(&mut value);
        assert_eq!(value, "aéz");
        edit.horizontal(&value, true, true);
        assert_eq!(&value[edit.range()], "é");
        edit.replace(&mut value, "筆");
        assert_eq!(value, "a筆z");
        edit.delete(&mut value);
        assert_eq!(value, "a筆");
    }

    #[test]
    fn multiline_navigation_and_selection_stay_on_characters() {
        let mut value = "aé🎨\nx\n雪山".to_string();
        let mut edit = TextEdit {
            cursor: "aé".len(),
            anchor: "aé".len(),
            ..TextEdit::default()
        };
        edit.vertical(&value, false, false);
        assert_eq!(edit.cursor, "aé🎨\nx".len());
        edit.vertical(&value, false, false);
        assert_eq!(edit.cursor, "aé🎨\nx\n雪".len());
        edit.home_end(&value, false, false, true);
        assert_eq!(&value[edit.range()], "雪");
        edit.replace(&mut value, "sun\nmoon");
        assert_eq!(value, "aé🎨\nx\nsun\nmoon山");
        edit.home_end(&value, false, true, false);
        assert_eq!(edit.cursor, 0);
        edit.home_end(&value, true, true, true);
        assert_eq!(edit.range(), 0..value.len());
    }

    #[test]
    fn menu_navigation_skips_unavailable_rows_and_wraps() {
        let selectable = [0, 2, 5];
        assert_eq!(next_menu_item(&selectable, Some(0), true), Some(5));
        assert_eq!(next_menu_item(&selectable, Some(0), false), Some(2));
        assert_eq!(next_menu_item(&selectable, Some(5), false), Some(0));
        assert_eq!(next_menu_item(&[], None, false), None);
    }
}
