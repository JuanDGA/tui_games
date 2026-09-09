//! Kitty keyboard protocol: enablement, event types, and CSI sequence parsing.
//!
//! Spec: <https://sw.kovidgoyal.net/kitty/keyboard-protocol/>
//!
//! Progressive enhancement flags this crate pushes:
//! - disambiguate escape codes
//! - report event types (press / repeat / release)
//! - report alternate keys (shifted + base layout)
//! - report all keys as escape codes (needed for WASD hold/release)
//!
//! Unsupported terminals ignore the push sequence and keep sending legacy bytes.

// Parser types are used from tests and as a byte-level API; the live event loop
// still goes through crossterm after we enable the protocol.
#![allow(dead_code)]

mod parse;

use std::io::{self, stdout};

use crossterm::event::{
    KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;

/// Flags pushed while the app is on the alternate screen.
pub const ENHANCEMENT_FLAGS: KeyboardEnhancementFlags =
    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        .union(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
        .union(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS)
        .union(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES);

/// Push kitty keyboard enhancement flags.
///
/// Safe to call on terminals that do not implement the protocol: they ignore the sequence.
pub fn enable() -> io::Result<()> {
    execute!(stdout(), PushKeyboardEnhancementFlags(ENHANCEMENT_FLAGS))
}

/// Pop one level of kitty keyboard enhancement flags.
pub fn disable() -> io::Result<()> {
    execute!(stdout(), PopKeyboardEnhancementFlags)
}

/// Press and repeat both fire gameplay actions. Release is tracked separately.
pub fn is_action(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

/// A parsed kitty keyboard protocol event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Key(KeyEvent),
    /// Reply to `CSI ? u` (current progressive-enhancement flags).
    FlagsReport(u16),
}

/// One key press, repeat, or release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    /// Shifted form of `key` in the active layout, when reported.
    pub shifted_key: Option<char>,
    /// Key in the standard PC-101 layout, when reported.
    pub base_layout_key: Option<char>,
    pub modifiers: Modifiers,
    pub event_type: EventType,
    /// Associated text as Unicode scalars, when reported.
    pub associated_text: String,
}

impl KeyEvent {
    pub fn is_press(&self) -> bool {
        self.event_type == EventType::Press
    }

    pub fn is_repeat(&self) -> bool {
        self.event_type == EventType::Repeat
    }

    pub fn is_release(&self) -> bool {
        self.event_type == EventType::Release
    }

    /// Press or repeat: the key is logically down for gameplay.
    pub fn is_action(&self) -> bool {
        matches!(self.event_type, EventType::Press | EventType::Repeat)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Press,
    Repeat,
    Release,
}

impl EventType {
    fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::Press),
            2 => Some(Self::Repeat),
            3 => Some(Self::Release),
            _ => None,
        }
    }
}

/// Modifier bitmask. Protocol encoding is `1 + bits`; this type stores the bits only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    bits: u8,
}

impl Modifiers {
    pub const NONE: Self = Self { bits: 0 };
    pub const SHIFT: Self = Self { bits: 1 << 0 };
    pub const ALT: Self = Self { bits: 1 << 1 };
    pub const CTRL: Self = Self { bits: 1 << 2 };
    pub const SUPER: Self = Self { bits: 1 << 3 };
    pub const HYPER: Self = Self { bits: 1 << 4 };
    pub const META: Self = Self { bits: 1 << 5 };
    pub const CAPS_LOCK: Self = Self { bits: 1 << 6 };
    pub const NUM_LOCK: Self = Self { bits: 1 << 7 };

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn contains(self, other: Self) -> bool {
        self.bits & other.bits == other.bits
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    pub const fn insert(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    /// Decode the protocol modifier field (`1 + actual modifiers`).
    pub const fn from_protocol(value: u32) -> Self {
        let bits = value.saturating_sub(1);
        Self {
            bits: if bits > u8::MAX as u32 {
                u8::MAX
            } else {
                bits as u8
            },
        }
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

/// The key that produced an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Un-shifted Unicode scalar for the physical key.
    Char(char),
    Functional(FunctionalKey),
    /// Code `0`: a text event with no known key.
    Unidentified,
    /// Code the parser does not map.
    Other(u32),
}

/// Non-Unicode keys from the protocol's functional-key table (PUA plus a few C0 codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionalKey {
    Escape,
    Enter,
    Tab,
    Backspace,
    Insert,
    Delete,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
    Menu,
    /// F1 through F35.
    F(u8),
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpDecimal,
    KpDivide,
    KpMultiply,
    KpSubtract,
    KpAdd,
    KpEnter,
    KpEqual,
    KpSeparator,
    KpLeft,
    KpRight,
    KpUp,
    KpDown,
    KpPageUp,
    KpPageDown,
    KpHome,
    KpEnd,
    KpInsert,
    KpDelete,
    KpBegin,
    MediaPlay,
    MediaPause,
    MediaPlayPause,
    MediaReverse,
    MediaStop,
    MediaFastForward,
    MediaRewind,
    MediaTrackNext,
    MediaTrackPrevious,
    MediaRecord,
    LowerVolume,
    RaiseVolume,
    MuteVolume,
    LeftShift,
    LeftControl,
    LeftAlt,
    LeftSuper,
    LeftHyper,
    LeftMeta,
    RightShift,
    RightControl,
    RightAlt,
    RightSuper,
    RightHyper,
    RightMeta,
    IsoLevel3Shift,
    IsoLevel5Shift,
}

impl FunctionalKey {
    fn from_u_code(code: u32) -> Option<Self> {
        Some(match code {
            9 => Self::Tab,
            13 => Self::Enter,
            27 => Self::Escape,
            127 => Self::Backspace,
            57358 => Self::CapsLock,
            57359 => Self::ScrollLock,
            57360 => Self::NumLock,
            57361 => Self::PrintScreen,
            57362 => Self::Pause,
            57363 => Self::Menu,
            57376..=57398 => Self::F((code - 57376 + 13) as u8),
            57399 => Self::Kp0,
            57400 => Self::Kp1,
            57401 => Self::Kp2,
            57402 => Self::Kp3,
            57403 => Self::Kp4,
            57404 => Self::Kp5,
            57405 => Self::Kp6,
            57406 => Self::Kp7,
            57407 => Self::Kp8,
            57408 => Self::Kp9,
            57409 => Self::KpDecimal,
            57410 => Self::KpDivide,
            57411 => Self::KpMultiply,
            57412 => Self::KpSubtract,
            57413 => Self::KpAdd,
            57414 => Self::KpEnter,
            57415 => Self::KpEqual,
            57416 => Self::KpSeparator,
            57417 => Self::KpLeft,
            57418 => Self::KpRight,
            57419 => Self::KpUp,
            57420 => Self::KpDown,
            57421 => Self::KpPageUp,
            57422 => Self::KpPageDown,
            57423 => Self::KpHome,
            57424 => Self::KpEnd,
            57425 => Self::KpInsert,
            57426 => Self::KpDelete,
            57427 => Self::KpBegin,
            57428 => Self::MediaPlay,
            57429 => Self::MediaPause,
            57430 => Self::MediaPlayPause,
            57431 => Self::MediaReverse,
            57432 => Self::MediaStop,
            57433 => Self::MediaFastForward,
            57434 => Self::MediaRewind,
            57435 => Self::MediaTrackNext,
            57436 => Self::MediaTrackPrevious,
            57437 => Self::MediaRecord,
            57438 => Self::LowerVolume,
            57439 => Self::RaiseVolume,
            57440 => Self::MuteVolume,
            57441 => Self::LeftShift,
            57442 => Self::LeftControl,
            57443 => Self::LeftAlt,
            57444 => Self::LeftSuper,
            57445 => Self::LeftHyper,
            57446 => Self::LeftMeta,
            57447 => Self::RightShift,
            57448 => Self::RightControl,
            57449 => Self::RightAlt,
            57450 => Self::RightSuper,
            57451 => Self::RightHyper,
            57452 => Self::RightMeta,
            57453 => Self::IsoLevel3Shift,
            57454 => Self::IsoLevel5Shift,
            _ => return None,
        })
    }

    fn from_tilde_number(number: u32) -> Option<Self> {
        Some(match number {
            2 => Self::Insert,
            3 => Self::Delete,
            5 => Self::PageUp,
            6 => Self::PageDown,
            7 => Self::Home,
            8 => Self::End,
            11 => Self::F(1),
            12 => Self::F(2),
            13 => Self::F(3),
            14 => Self::F(4),
            15 => Self::F(5),
            17 => Self::F(6),
            18 => Self::F(7),
            19 => Self::F(8),
            20 => Self::F(9),
            21 => Self::F(10),
            23 => Self::F(11),
            24 => Self::F(12),
            29 => Self::Menu,
            other => return Self::from_u_code(other),
        })
    }

    fn from_letter(letter: u8) -> Option<Self> {
        Some(match letter {
            b'A' => Self::Up,
            b'B' => Self::Down,
            b'C' => Self::Right,
            b'D' => Self::Left,
            b'E' => Self::KpBegin,
            b'F' => Self::End,
            b'H' => Self::Home,
            b'P' => Self::F(1),
            b'Q' => Self::F(2),
            b'R' => Self::F(3),
            b'S' => Self::F(4),
            b'Z' => Self::Tab,
            _ => return None,
        })
    }
}

fn key_from_u_code(code: u32) -> Key {
    if code == 0 {
        return Key::Unidentified;
    }
    if let Some(func) = FunctionalKey::from_u_code(code) {
        return Key::Functional(func);
    }
    if let Some(ch) = char::from_u32(code) {
        return Key::Char(ch);
    }
    Key::Other(code)
}

fn optional_char_code(code: u32) -> Option<char> {
    if code == 0 {
        return None;
    }
    char::from_u32(code)
}

#[cfg(test)]
mod tests {
    use super::parse::{Decoder, Parse, ParseError, parse, parse_sequence};
    use super::*;

    #[test]
    fn enhancement_flags_match_spec_bits() {
        assert_eq!(ENHANCEMENT_FLAGS.bits(), 0b1111);
    }

    #[test]
    fn action_includes_press_and_repeat_not_release() {
        assert!(is_action(KeyEventKind::Press));
        assert!(is_action(KeyEventKind::Repeat));
        assert!(!is_action(KeyEventKind::Release));
    }

    #[test]
    fn modifiers_decode_protocol_offset() {
        assert_eq!(Modifiers::from_protocol(1), Modifiers::NONE);
        assert_eq!(Modifiers::from_protocol(2), Modifiers::SHIFT);
        assert_eq!(
            Modifiers::from_protocol(6),
            Modifiers::SHIFT | Modifiers::CTRL
        );
        assert_eq!(Modifiers::from_protocol(9), Modifiers::SUPER);
        assert_eq!(Modifiers::CTRL.bits(), 4);
    }

    #[test]
    fn parse_sequence_exposes_press_repeat_release() {
        let press = match parse_sequence(b"\x1b[97u").unwrap() {
            Event::Key(event) => event,
            other => panic!("unexpected {other:?}"),
        };
        assert!(press.is_press());
        assert!(!press.is_repeat());
        assert!(!press.is_release());

        let repeat = match parse_sequence(b"\x1b[97;1:2u").unwrap() {
            Event::Key(event) => event,
            other => panic!("unexpected {other:?}"),
        };
        assert!(repeat.is_repeat());

        let release = match parse_sequence(b"\x1b[97;1:3u").unwrap() {
            Event::Key(event) => event,
            other => panic!("unexpected {other:?}"),
        };
        assert!(release.is_release());
    }

    #[test]
    fn decoder_and_parse_skip_legacy_bytes() {
        assert!(matches!(parse(b"q").unwrap(), Parse::Skip(1)));

        let mut decoder = Decoder::new();
        decoder.feed(b"\x1b[97u");
        assert!(decoder.decode().unwrap().is_some());
        assert!(matches!(
            parse_sequence(b"not-csi"),
            Err(ParseError::Unsupported)
        ));
    }
}
