// SPDX-License-Identifier: AGPL-3.0-or-later

//! Client-side decorations: drag areas, resize edges and the window controls. There's no titlebar;
//! the sidebar's logo row and a strip along the top of the content move the window.

use iced_core::Length::{self, Fill};
use iced_core::Point;
use iced_core::mouse::Interaction;
use iced_core::window::Direction;
use iced_widget::{Element, Widget, column, container, mouse_area, row, space};

use crate::ui::Look;
use crate::ui::icon::{Icon, icon};
use crate::ui::pressable::{Appearance, Status, pressable};
use crate::ui::squircle::{Paint, squircle};

/// The invisible strip along the top of the content that moves the window.
pub const DRAG_STRIP: f32 = 12.0;
/// How far in from the window's edge resizing starts.
const EDGE: f32 = 6.0;
/// How far along each side a corner reaches.
const CORNER: f32 = 14.0;
/// How far the pointer has to move with the button down before a press becomes a move.
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Minimize,
    Maximize,
    Close,
}

/// Which controls go on which side, from the desktop's `button-layout` (GNOME's format, which the
/// settings portal also uses for KDE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonLayout {
    pub left: Vec<Control>,
    pub right: Vec<Control>,
}

impl Default for ButtonLayout {
    fn default() -> Self {
        Self { left: Vec::new(), right: vec![Control::Minimize, Control::Maximize, Control::Close] }
    }
}

impl ButtonLayout {
    /// Parses a layout like `appmenu:minimize,maximize,close`. Everything before the colon is on
    /// the left; without a colon, everything is.
    pub fn parse(layout: &str) -> Self {
        let side = |s: &str| {
            s.split(',')
                .filter_map(|name| match name.trim() {
                    "minimize" => Some(Control::Minimize),
                    "maximize" => Some(Control::Maximize),
                    "close" => Some(Control::Close),
                    _ => None,
                })
                .collect()
        };

        match layout.split_once(':') {
            Some((left, right)) => Self { left: side(left), right: side(right) },
            None => Self { left: side(layout), right: Vec::new() },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum Message {
    Press,
    Move(Point),
    Release,
    DoubleClick,
    Menu,
    Resize(Direction),
    Control(Control),
}

/// What the window should do, carried out by the event loop.
#[derive(Debug, Clone, Copy)]
pub enum Action {
    Drag,
    Resize(Direction),
    ToggleMaximize,
    Minimize,
    Close,
    Menu,
    Fullscreen(bool),
}

#[derive(Debug, Default)]
pub struct Chrome {
    pub layout: ButtonLayout,
    pub maximized: bool,
    pub fullscreen: bool,
    /// Where a press on a drag area started, until it moves far enough to become a window move.
    pressed: Option<Option<Point>>,
}

impl Chrome {
    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::Press => {
                self.pressed = Some(None);
                None
            },
            Message::Move(point) => match self.pressed {
                Some(None) => {
                    self.pressed = Some(Some(point));
                    None
                },
                Some(Some(start)) if start.distance(point) >= DRAG_THRESHOLD => {
                    self.pressed = None;
                    Some(Action::Drag)
                },
                _ => None,
            },
            Message::Release => {
                self.pressed = None;
                None
            },
            Message::DoubleClick => {
                self.pressed = None;
                Some(Action::ToggleMaximize)
            },
            Message::Menu => Some(Action::Menu),
            Message::Resize(direction) => Some(Action::Resize(direction)),
            Message::Control(Control::Minimize) => Some(Action::Minimize),
            Message::Control(Control::Maximize) => Some(Action::ToggleMaximize),
            Message::Control(Control::Close) => Some(Action::Close),
        }
    }

    /// Hidden while the window is fullscreen, where they'd be in the way.
    pub fn controls_visible(&self) -> bool {
        !self.fullscreen
    }

    fn resizable(&self) -> bool {
        !self.maximized && !self.fullscreen
    }

    /// Makes `content` move the window when dragged, maximize it on a double click and open the
    /// compositor's window menu on a right click. Anything interactive inside keeps working.
    pub fn drag_area<'a, M: Clone + 'a>(
        &self,
        content: impl Widget<M> + 'a,
        to: impl Fn(Message) -> M + Copy + 'a,
    ) -> Element<'a, M> {
        mouse_area(content)
            .on_press(to(Message::Press))
            .on_move(move |point| to(Message::Move(point)))
            .on_release(to(Message::Release))
            .on_double_click(to(Message::DoubleClick))
            .on_right_press(to(Message::Menu))
            .boxed()
    }

    /// The controls for one side, or nothing if the layout puts none there.
    pub fn controls<'a>(&self, side: Side, look: &Look) -> Option<Element<'a, Message>> {
        let controls = match side {
            Side::Left => &self.layout.left,
            Side::Right => &self.layout.right,
        };
        if controls.is_empty() || !self.controls_visible() {
            return None;
        }

        let maximized = self.maximized;
        let l = *look;
        let buttons = controls.iter().map(|&control| {
            let glyph = match control {
                Control::Minimize => Icon::Minus,
                Control::Maximize if maximized => Icon::Copy,
                Control::Maximize => Icon::Square,
                Control::Close => Icon::X,
            };
            let close = control == Control::Close;
            pressable(container(icon(glyph, 14.0).stroke(1.6)).center(28), 14.0, look, move |status| {
                let (fill, ink) = match status {
                    Status::Hovered if close => (Some(l.danger), l.canvas),
                    Status::Pressed if close => (Some(l.danger.scale_alpha(0.8)), l.canvas),
                    Status::Hovered => (Some(l.hover), l.ink),
                    Status::Pressed => (Some(l.press), l.ink),
                    _ => (None, l.ink_2),
                };
                Appearance { paint: Paint { fill, ..Paint::NONE }, ink }
            })
            .round()
            .on_press(Message::Control(control))
            .boxed()
        });

        Some(row(buttons).spacing(2).boxed())
    }

    /// The controls in their floating pill, of the same material as menus.
    pub fn pill<'a>(&self, side: Side, look: &Look, alpha: f32) -> Option<Element<'a, Message>> {
        let alpha = alpha.clamp(0.0, 1.0);
        if alpha == 0.0 {
            return None;
        }
        let material = look.material().map(|mut material| {
            material.blur *= alpha;
            material.saturate = 1.0 + (material.saturate - 1.0) * alpha;
            material.tint = material.tint.scale_alpha(alpha);
            material.bezel *= alpha;
            material
        });
        let mut look = *look;
        for color in [
            &mut look.float,
            &mut look.ink,
            &mut look.ink_2,
            &mut look.canvas,
            &mut look.hover,
            &mut look.press,
            &mut look.danger,
            &mut look.glow,
            &mut look.line_strong,
            &mut look.shade,
        ] {
            *color = color.scale_alpha(alpha);
        }
        self.controls(side, &look).map(|controls| {
            squircle(controls, 17.0, &look)
                .round()
                .paint(Paint { fill: Some(look.float), material, edge: true, lift: true, ..Paint::NONE })
                .padding(3)
                .boxed()
        })
    }

    /// Invisible resize handles around the window, layered over everything else.
    pub fn edges<'a>(&self) -> Element<'a, Message> {
        if !self.resizable() {
            return space().boxed();
        }

        let handle = |direction, interaction, width: Length, height: Length| {
            mouse_area(space().width(width).height(height))
                .interaction(interaction)
                .on_press(Message::Resize(direction))
        };
        let side = |end: Direction, middle: Direction, other: Direction| {
            column![
                handle(end, corner(end), Fill, Length::Fixed(CORNER - EDGE)),
                handle(middle, Interaction::ResizingHorizontally, Fill, Fill),
                handle(other, corner(other), Fill, Length::Fixed(CORNER - EDGE)),
            ]
            .width(EDGE)
        };
        let top_or_bottom = |start: Direction, middle: Direction, end: Direction| {
            row![
                handle(start, corner(start), Length::Fixed(CORNER), Fill),
                handle(middle, Interaction::ResizingVertically, Fill, Fill),
                handle(end, corner(end), Length::Fixed(CORNER), Fill),
            ]
            .height(EDGE)
        };

        column![
            top_or_bottom(Direction::NorthWest, Direction::North, Direction::NorthEast),
            row![
                side(Direction::NorthWest, Direction::West, Direction::SouthWest),
                space().width(Fill).height(Fill),
                side(Direction::NorthEast, Direction::East, Direction::SouthEast),
            ]
            .height(Fill),
            top_or_bottom(Direction::SouthWest, Direction::South, Direction::SouthEast),
        ]
        .boxed()
    }
}

fn corner(direction: Direction) -> Interaction {
    match direction {
        Direction::NorthEast | Direction::SouthWest => Interaction::ResizingDiagonallyUp,
        _ => Interaction::ResizingDiagonallyDown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_layouts() {
        assert_eq!(ButtonLayout::parse("appmenu:close"), ButtonLayout { left: vec![], right: vec![Control::Close] });
        assert_eq!(
            ButtonLayout::parse("close,minimize,maximize:"),
            ButtonLayout { left: vec![Control::Close, Control::Minimize, Control::Maximize], right: vec![] }
        );
        assert_eq!(
            ButtonLayout::parse("icon:minimize, maximize ,close"),
            ButtonLayout { left: vec![], right: vec![Control::Minimize, Control::Maximize, Control::Close] }
        );
        assert_eq!(ButtonLayout::parse("close"), ButtonLayout { left: vec![Control::Close], right: vec![] });
        assert_eq!(ButtonLayout::parse(""), ButtonLayout { left: vec![], right: vec![] });
    }

    #[test]
    fn hides_controls_when_fullscreen() {
        let chrome = Chrome { fullscreen: true, ..Chrome::default() };

        assert!(chrome.controls(Side::Right, &Look::default()).is_none());
        assert!(Chrome::default().controls(Side::Right, &Look::default()).is_some());
    }
}
