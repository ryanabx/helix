use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
};

use alacritty_terminal::{
    event::{Event, EventListener},
    term::{test::TermSize, Config},
    vte::ansi,
    Term,
};

use helix_vte::{PtyEvent, TerminalId, VteRegistry};
use termwiz::{input::{KeyCodeEncodeModes, KeyboardEncoding}, terminal::Terminal};
use tokio::{select, sync::mpsc};
use tokio_stream::StreamExt;

use crate::{
    graphics::{Color, CursorKind},
    input::{self, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent},
};

impl From<ansi::CursorShape> for CursorKind {
    fn from(shape: ansi::CursorShape) -> Self {
        match shape {
            ansi::CursorShape::Block => CursorKind::Block,
            ansi::CursorShape::Underline => CursorKind::Underline,
            ansi::CursorShape::Beam => CursorKind::Bar,
            ansi::CursorShape::HollowBlock => CursorKind::Block,
            ansi::CursorShape::Hidden => CursorKind::Hidden,
        }
    }
}

impl From<ansi::Color> for Color {
    fn from(col: ansi::Color) -> Self {
        match col {
            ansi::Color::Named(named) => match named {
                ansi::NamedColor::Black => Color::Black,
                ansi::NamedColor::Red => Color::Red,
                ansi::NamedColor::Green => Color::Green,
                ansi::NamedColor::Yellow => Color::Yellow,
                ansi::NamedColor::Blue => Color::Blue,
                ansi::NamedColor::Magenta => Color::Magenta,
                ansi::NamedColor::Cyan => Color::Cyan,
                ansi::NamedColor::White => Color::White,
                ansi::NamedColor::BrightBlack => Color::Gray,
                ansi::NamedColor::BrightRed => Color::LightRed,
                ansi::NamedColor::BrightGreen => Color::LightGreen,
                ansi::NamedColor::BrightYellow => Color::LightYellow,
                ansi::NamedColor::BrightBlue => Color::LightBlue,
                ansi::NamedColor::BrightMagenta => Color::LightMagenta,
                ansi::NamedColor::BrightCyan => Color::LightCyan,
                _ => Color::Reset,
            },
            ansi::Color::Spec(c) => Color::Rgb(c.r, c.g, c.b),
            ansi::Color::Indexed(idx) => Color::Indexed(idx),
        }
    }
}

pub struct Listener {
    term_id: TerminalId,
    sender: mpsc::UnboundedSender<(TerminalId, Event)>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let _ = self.sender.send((self.term_id, event));
    }
}

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    TitleChange(TerminalId, String),
    Update(TerminalId),
}

pub enum TerminalState {
    Initializing,
    Normal,
    Failed(String),
    Terminated(i32),
}

#[derive(Eq, PartialEq)]
pub enum ChordState {
    Normal,
    Quit1,
}

struct TerminalModel {
    state: TerminalState,
    parser: termwiz::escape::parser::Parser,
    surface: termwiz::surface::Surface,
    term: Term<Listener>,
    input_parser: termwiz::input::InputParser,
}

impl TerminalModel {
    #[inline]
    fn advance(&mut self, data: &[u8]) {
        self.parser.parse(&data, |action| {});
    }

    #[inline]
    fn resize(&mut self, size: (u16, u16)) {
        self.surface.resize(size.1 as _, size.0 as _);
    }
}

fn encode_from_input(input: &termwiz::input::InputEvent) -> Vec<u8> {
    match input {
        termwiz::input::InputEvent::Key(key_event) => key_event.key.encode(key_event.modifiers, KeyCodeEncodeModes {
            
        }, is_down),
        termwiz::input::InputEvent::Mouse(mouse_event) => todo!(),
        termwiz::input::InputEvent::PixelMouse(pixel_mouse_event) => todo!(),
        termwiz::input::InputEvent::Resized { cols, rows } => todo!(),
        termwiz::input::InputEvent::Paste(_) => todo!(),
        termwiz::input::InputEvent::Wake => todo!(),
    }
}

fn input_from_input(input: &input::Event) -> termwiz::input::InputEvent {
    match input {
        input::Event::FocusGained => todo!("input::Event::FocusGained"),
        input::Event::FocusLost => todo!("input::Event::FocusLost"),
        input::Event::Key(key_event) => {
            termwiz::input::InputEvent::Key(self::key_event_to_termwiz(key_event))
        }
        input::Event::Mouse(mouse_event) => {
            termwiz::input::InputEvent::Mouse(termwiz::input::MouseEvent {
                x: mouse_event.column,
                y: mouse_event.row,
                modifiers: self::key_modifiers_to_termwiz(&mouse_event.modifiers),
                mouse_buttons: {
                    match mouse_event.kind {
                        input::MouseEventKind::Down(mouse_button) => {
                            self::mouse_button_to_termwiz(&mouse_button)
                        }
                        input::MouseEventKind::Up(_mouse_button) => {
                            termwiz::input::MouseButtons::NONE
                        }
                        input::MouseEventKind::Drag(mouse_button) => {
                            self::mouse_button_to_termwiz(&mouse_button)
                        }
                        input::MouseEventKind::Moved => termwiz::input::MouseButtons::NONE,
                        input::MouseEventKind::ScrollDown => {
                            termwiz::input::MouseButtons::VERT_WHEEL
                        }
                        input::MouseEventKind::ScrollUp => termwiz::input::MouseButtons::VERT_WHEEL,
                        input::MouseEventKind::ScrollLeft => {
                            termwiz::input::MouseButtons::HORZ_WHEEL
                        }
                        input::MouseEventKind::ScrollRight => {
                            termwiz::input::MouseButtons::HORZ_WHEEL
                        }
                    }
                },
            })
        }
        input::Event::Paste(content) => termwiz::input::InputEvent::Paste(content),
        input::Event::Resize(cols, rows) => termwiz::input::InputEvent::Resized {
            cols: *cols as _,
            rows: *rows as _,
        },
        input::Event::IdleTimeout => todo!("input::Event::IdleTimeout"),
    }
}

fn key_event_to_termwiz(evt: &KeyEvent) -> termwiz::input::KeyEvent {
    termwiz::input::KeyEvent {
        key: self::key_code_to_termwiz(&evt.code),
        modifiers: self::key_modifiers_to_termwiz(&evt.modifiers),
    }
}

fn key_code_to_termwiz(code: &KeyCode) -> termwiz::input::KeyCode {
    match code {
        KeyCode::Backspace => termwiz::input::KeyCode::Backspace,
        KeyCode::Enter => termwiz::input::KeyCode::Enter,
        KeyCode::Left => termwiz::input::KeyCode::LeftArrow,
        KeyCode::Right => termwiz::input::KeyCode::RightArrow,
        KeyCode::Up => termwiz::input::KeyCode::UpArrow,
        KeyCode::Down => termwiz::input::KeyCode::DownArrow,
        KeyCode::Home => termwiz::input::KeyCode::Home,
        KeyCode::End => termwiz::input::KeyCode::End,
        KeyCode::PageUp => termwiz::input::KeyCode::PageUp,
        KeyCode::PageDown => termwiz::input::KeyCode::PageDown,
        KeyCode::Tab => termwiz::input::KeyCode::Tab,
        KeyCode::Delete => termwiz::input::KeyCode::Delete,
        KeyCode::Insert => termwiz::input::KeyCode::Insert,
        KeyCode::F(f) => termwiz::input::KeyCode::Function(f),
        KeyCode::Char(c) => termwiz::input::KeyCode::Char(c),
        KeyCode::Null => todo!("termwiz::input::KeyCode::Null"),
        KeyCode::Esc => termwiz::input::KeyCode::Escape,
        KeyCode::CapsLock => termwiz::input::KeyCode::CapsLock,
        KeyCode::ScrollLock => termwiz::input::KeyCode::ScrollLock,
        KeyCode::NumLock => termwiz::input::KeyCode::NumLock,
        KeyCode::PrintScreen => termwiz::input::KeyCode::PrintScreen,
        KeyCode::Pause => termwiz::input::KeyCode::Pause,
        KeyCode::Menu => termwiz::input::KeyCode::Menu,
        KeyCode::KeypadBegin => termwiz::input::KeyCode::KeyPadBegin,
        KeyCode::Media(media_key_code) => match media_key_code {
            input::MediaKeyCode::Play => todo!("termwiz::input::KeyCode::MediaPlay"),
            input::MediaKeyCode::Pause => todo!("termwiz::input::KeyCode::MediaPause"),
            input::MediaKeyCode::PlayPause => termwiz::input::KeyCode::MediaPlayPause,
            input::MediaKeyCode::Reverse => todo!("termwiz::input::KeyCode::Reverse"),
            input::MediaKeyCode::Stop => termwiz::input::KeyCode::MediaStop,
            input::MediaKeyCode::FastForward => todo!("termwiz::input::KeyCode::FastForward"),
            input::MediaKeyCode::Rewind => todo!("termwiz::input::KeyCode::Rewind"),
            input::MediaKeyCode::TrackNext => termwiz::input::KeyCode::MediaNextTrack,
            input::MediaKeyCode::TrackPrevious => termwiz::input::KeyCode::MediaPrevTrack,
            input::MediaKeyCode::Record => todo!("termwiz::input::KeyCode::Record"),
            input::MediaKeyCode::LowerVolume => termwiz::input::KeyCode::VolumeDown,
            input::MediaKeyCode::RaiseVolume => termwiz::input::KeyCode::VolumeUp,
            input::MediaKeyCode::MuteVolume => termwiz::input::KeyCode::VolumeMute,
        },
        KeyCode::Modifier(modifier_key_code) => match modifier_key_code {
            input::ModifierKeyCode::LeftShift => todo!(),
            input::ModifierKeyCode::LeftControl => todo!(),
            input::ModifierKeyCode::LeftAlt => todo!(),
            input::ModifierKeyCode::LeftSuper => todo!(),
            input::ModifierKeyCode::LeftHyper => todo!(),
            input::ModifierKeyCode::LeftMeta => todo!(),
            input::ModifierKeyCode::RightShift => todo!(),
            input::ModifierKeyCode::RightControl => todo!(),
            input::ModifierKeyCode::RightAlt => todo!(),
            input::ModifierKeyCode::RightSuper => todo!(),
            input::ModifierKeyCode::RightHyper => todo!(),
            input::ModifierKeyCode::RightMeta => todo!(),
            input::ModifierKeyCode::IsoLevel3Shift => todo!(),
            input::ModifierKeyCode::IsoLevel5Shift => todo!(),
        },
    }
}

fn mouse_button_to_termwiz(button: &MouseButton) -> termwiz::input::MouseButtons {
    match button {
        MouseButton::Left => termwiz::input::MouseButtons::LEFT,
        MouseButton::Right => termwiz::input::MouseButtons::RIGHT,
        MouseButton::Middle => termwiz::input::MouseButtons::MIDDLE,
    }
}

fn key_modifiers_to_termwiz(modifiers: &KeyModifiers) -> termwiz::input::Modifiers {
    let mut modifiers_ret = termwiz::input::Modifiers::empty();
    if modifiers.contains(KeyModifiers::ALT) {
        modifiers_ret.insert(termwiz::input::Modifiers::ALT);
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        modifiers_ret.insert(termwiz::input::Modifiers::CTRL);
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        modifiers_ret.insert(termwiz::input::Modifiers::SHIFT);
    }
    if modifiers.contains(KeyModifiers::SUPER) {
        modifiers_ret.insert(termwiz::input::Modifiers::SUPER);
    }
    if modifiers.contains(KeyModifiers::NONE) {
        modifiers_ret.insert(termwiz::input::Modifiers::NONE);
    }
    modifiers_ret
}

pub struct TerminalView {
    config: Config,
    chord_state: ChordState,
    pub visible: bool,
    pub viewport: (u16, u16),
    active_term: Option<TerminalId>,
    events: mpsc::UnboundedReceiver<(TerminalId, Event)>,
    sender: mpsc::UnboundedSender<(TerminalId, Event)>,
    pub(crate) registry: VteRegistry,
    models: HashMap<TerminalId, RefCell<TerminalModel>>,
}

impl TerminalView {
    pub fn new() -> TerminalView {
        let (sender, events) = mpsc::unbounded_channel();

        Self {
            config: Config::default(),
            chord_state: ChordState::Normal,
            active_term: None,
            visible: false,
            viewport: (24, 80),
            events,
            sender,
            registry: VteRegistry::new(),
            models: Default::default(),
        }
    }

    pub fn spawn_shell(&mut self, size: (u16, u16)) {
        if let Ok(term_id) = self.registry.spawn_pty(Default::default()) {
            let sender = self.sender.clone();
            let listener = Listener { term_id, sender };

            let size = TermSize::new(size.1 as _, size.0 as _);
            self.active_term = Some(term_id);
            self.models.insert(
                term_id,
                RefCell::new(TerminalModel {
                    state: TerminalState::Initializing,
                    parser: ansi::Processor::new(),
                    term: Term::new(self.config.clone(), &size, listener),
                }),
            );
        }
    }

    pub fn toggle_terminal(&mut self) {
        if self.active_term.is_none() {
            self.spawn_shell(self.viewport);
        }

        if let Some(term_id) = self.active_term {
            self.visible = !self.visible;
            let _ = self.sender.send((term_id, Event::Wakeup));
        }
    }

    #[inline]
    pub fn close_active_terminal(&mut self) {
        if let Some(term_id) = self.active_term {
            self.close_term(term_id)
        }
    }

    #[inline]
    pub fn get_active(&'_ self) -> Option<(TerminalId, Ref<'_, Term<Listener>>)> {
        let id = self.active_term?;

        Some((id, self.get_term(id)?))
    }

    pub fn get_active_mut(&'_ mut self) -> Option<(TerminalId, RefMut<'_, Term<Listener>>)> {
        let id = self.active_term?;

        Some((id, self.get_term_mut(id)?))
    }

    #[inline]
    pub fn get_term(&'_ self, id: TerminalId) -> Option<Ref<'_, Term<Listener>>> {
        self.models
            .get(&id)
            .map(|t| Ref::map(t.borrow(), |x| &x.term))
    }

    #[inline]
    pub fn get_term_mut(&'_ self, id: TerminalId) -> Option<RefMut<'_, Term<Listener>>> {
        self.models
            .get(&id)
            .map(|t| RefMut::map(t.borrow_mut(), |x| &mut x.term))
    }

    pub fn close_term(&mut self, id: TerminalId) {
        if let Some(mut term) = self.models.remove(&id) {
            if !matches!(
                term.get_mut().state,
                TerminalState::Failed(_) | TerminalState::Terminated(_)
            ) {
                let _ = self.registry.terminate(id);
            }

            drop(term)
        }
    }

    async fn handle_input_event_async(
        &mut self,
        id: TerminalId,
        event: &input::Event,
    ) -> Result<(), helix_vte::error::Error> {
        let event = input_from_input(event);
        
        self.registry.write()
        Ok(())
    }

    pub fn handle_input_event(&mut self, event: &input::Event) -> bool {
        if let Some(id) = self.active_term {
            let _res = helix_lsp::block_on(self.handle_input_event_async(id, event));
            return true;
        }

        false
    }

    async fn handle_mouse_event(
        &mut self,
        _id: TerminalId,
        _evt: MouseEvent,
    ) -> Result<(), helix_vte::error::Error> {
        if let Some((_id, _term)) = self.get_active_mut() {}

        Ok(())
    }

    pub async fn poll_event(&mut self) -> Option<TerminalEvent> {
        select!(
            event = self.events.recv() => {
                let (id, event) = event?;

                match event {
                    Event::Wakeup => Some(TerminalEvent::Update(id)),
                    Event::Title(title) => Some(TerminalEvent::TitleChange(id, title)),
                    Event::PtyWrite(data) => {
                        let _ = self.registry.write(id, data).await;
                        None
                    }

                    // ResetTitle,
                    // ClipboardStore(ClipboardType, String),
                    // ClipboardLoad(ClipboardType, Arc<dyn Fn(&str) -> String + Sync + Send + 'static>),
                    // MouseCursorDirty => ,
                    // ColorRequest(usize, Arc<dyn Fn(Rgb) -> String + Sync + Send + 'static>),
                    // TextAreaSizeRequest(Arc<dyn Fn(WindowSize) -> String + Sync + Send + 'static>),
                    // CursorBlinkingChange,
                    // Wakeup,
                    // Bell,
                    // Exit,
                    _ => None
                }
            }

            event = self.registry.incoming.next() => {
                let (id, event) = event?;

                match event {
                    PtyEvent::Data(data) => {
                        self.models.get(&id)?.borrow_mut().advance(data);
                        Some(TerminalEvent::Update(id))
                    }
                    PtyEvent::Error(err) => {
                        let term = self.models.get_mut(&id)?;
                        term.get_mut().state = TerminalState::Failed(err);
                        Some(TerminalEvent::Update(id))
                    }
                    PtyEvent::Terminated(code) => {
                        let term = self.models.get_mut(&id)?;
                        term.get_mut().state = TerminalState::Terminated(code);
                        self.active_term = None;
                        self.visible = false;
                        Some(TerminalEvent::Update(id))
                    }
                }

            }
        )
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}
