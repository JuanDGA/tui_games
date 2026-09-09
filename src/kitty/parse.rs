//! Incremental and one-shot parsing of kitty keyboard protocol sequences.

use super::{
    Event, EventType, FunctionalKey, Key, KeyEvent, Modifiers, key_from_u_code, optional_char_code,
};

const ESC: u8 = 0x1b;
const MAX_SEQUENCE: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Sequence is complete but not a kitty key / flags event.
    Unsupported,
    /// Parameter bytes were not decimal numbers / colons / semicolons.
    InvalidParams,
    /// A required field was missing or out of range.
    InvalidField,
    /// Sequence exceeded [`MAX_SEQUENCE`] without a final byte.
    Overflow,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "not a kitty keyboard sequence"),
            Self::InvalidParams => write!(f, "invalid kitty keyboard parameters"),
            Self::InvalidField => write!(f, "invalid kitty keyboard field"),
            Self::Overflow => write!(f, "kitty keyboard sequence too long"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Result of pulling one event from a byte buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Parse {
    /// Need more bytes to finish the sequence starting at `input[0]`.
    Incomplete,
    /// `n` leading bytes are not a kitty sequence (legacy text, other CSI, etc.).
    Skip(usize),
    Event {
        event: Event,
        consumed: usize,
    },
}

/// Parse the start of `input` as a kitty keyboard protocol event.
///
/// `Ok(Parse::Incomplete)` means `input` is a valid prefix. `Ok(Parse::Skip(n))`
/// means the first `n` bytes should be discarded by a stream decoder.
pub fn parse(input: &[u8]) -> Result<Parse, ParseError> {
    if input.is_empty() {
        return Ok(Parse::Incomplete);
    }
    if input[0] != ESC {
        return Ok(Parse::Skip(utf8_len(input[0])));
    }
    if input.len() == 1 {
        return Ok(Parse::Incomplete);
    }

    match input[1] {
        b'O' => parse_ss3(input),
        b'[' => parse_csi(input),
        _ => Ok(Parse::Skip(1)),
    }
}

/// Parse a single complete sequence. Errors if the buffer is incomplete or has trailing bytes.
pub fn parse_sequence(input: &[u8]) -> Result<Event, ParseError> {
    match parse(input)? {
        Parse::Event { event, consumed } if consumed == input.len() => Ok(event),
        Parse::Event { .. } => Err(ParseError::Unsupported),
        Parse::Incomplete => Err(ParseError::InvalidField),
        Parse::Skip(_) => Err(ParseError::Unsupported),
    }
}

/// Accumulates bytes and yields kitty key events, skipping non-kitty input.
#[derive(Debug, Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Decode the next kitty event. `Ok(None)` means more bytes are needed.
    pub fn decode(&mut self) -> Result<Option<Event>, ParseError> {
        loop {
            if self.buf.is_empty() {
                return Ok(None);
            }
            match parse(&self.buf)? {
                Parse::Incomplete => {
                    if self.buf.len() >= MAX_SEQUENCE {
                        self.buf.clear();
                        return Err(ParseError::Overflow);
                    }
                    return Ok(None);
                }
                Parse::Skip(n) => {
                    let n = n.min(self.buf.len()).max(1);
                    self.buf.drain(..n);
                }
                Parse::Event { event, consumed } => {
                    self.buf.drain(..consumed);
                    return Ok(Some(event));
                }
            }
        }
    }
}

fn parse_ss3(input: &[u8]) -> Result<Parse, ParseError> {
    // SS3 is ESC O followed by a final byte (arrows, Home/End, F1–F4).
    if input.len() < 3 {
        return Ok(Parse::Incomplete);
    }
    let Some(key) = FunctionalKey::from_letter(input[2]).map(Key::Functional) else {
        return Ok(Parse::Skip(1));
    };
    Ok(Parse::Event {
        event: Event::Key(KeyEvent {
            key,
            shifted_key: None,
            base_layout_key: None,
            modifiers: Modifiers::NONE,
            event_type: EventType::Press,
            associated_text: String::new(),
        }),
        consumed: 3,
    })
}

fn parse_csi(input: &[u8]) -> Result<Parse, ParseError> {
    // CSI: ESC [ [private] parameters [intermediates] final
    if input.len() < 3 {
        return Ok(Parse::Incomplete);
    }

    let mut i = 2;
    let private = if matches!(input[i], b'<' | b'=' | b'>' | b'?') {
        let p = input[i];
        i += 1;
        Some(p)
    } else {
        None
    };

    while i < input.len() && (0x30..=0x3f).contains(&input[i]) {
        i += 1;
    }
    while i < input.len() && (0x20..=0x2f).contains(&input[i]) {
        i += 1;
    }
    if i >= input.len() {
        if input.len() >= MAX_SEQUENCE {
            return Err(ParseError::Overflow);
        }
        return Ok(Parse::Incomplete);
    }

    let final_byte = input[i];
    if !(0x40..=0x7e).contains(&final_byte) {
        return Ok(Parse::Skip(1));
    }
    let consumed = i + 1;
    let params_bytes = match private {
        Some(_) => &input[3..i],
        None => &input[2..i],
    };

    if private == Some(b'?') && final_byte == b'u' {
        let flags = parse_flags_report(params_bytes)?;
        return Ok(Parse::Event {
            event: Event::FlagsReport(flags),
            consumed,
        });
    }

    if private.is_some() {
        return Ok(Parse::Skip(consumed));
    }

    let event = match final_byte {
        b'u' => parse_u_sequence(params_bytes)?,
        b'~' => parse_tilde_sequence(params_bytes)?,
        letter if FunctionalKey::from_letter(letter).is_some() => {
            parse_letter_sequence(params_bytes, letter)?
        }
        _ => return Ok(Parse::Skip(consumed)),
    };

    Ok(Parse::Event {
        event: Event::Key(event),
        consumed,
    })
}

fn parse_flags_report(params: &[u8]) -> Result<u16, ParseError> {
    if params.is_empty() {
        return Ok(0);
    }
    let fields = split_params(params)?;
    let first = fields.first().and_then(|p| p.first()).copied().flatten();
    first
        .map(|n| u16::try_from(n).map_err(|_| ParseError::InvalidField))
        .unwrap_or(Ok(0))
}

fn parse_u_sequence(params: &[u8]) -> Result<KeyEvent, ParseError> {
    let fields = split_params(params)?;
    let key_field = fields.first().ok_or(ParseError::InvalidField)?;
    let code = key_field
        .first()
        .copied()
        .flatten()
        .ok_or(ParseError::InvalidField)?;
    let key = key_from_u_code(code);

    let shifted_key = key_field
        .get(1)
        .copied()
        .flatten()
        .and_then(optional_char_code);
    let base_layout_key = key_field
        .get(2)
        .copied()
        .flatten()
        .and_then(optional_char_code);

    let (modifiers, event_type) = modifiers_and_type(fields.get(1))?;
    let associated_text = associated_text(fields.get(2))?;

    Ok(KeyEvent {
        key,
        shifted_key,
        base_layout_key,
        modifiers,
        event_type,
        associated_text,
    })
}

fn parse_tilde_sequence(params: &[u8]) -> Result<KeyEvent, ParseError> {
    let fields = split_params(params)?;
    let number = fields
        .first()
        .and_then(|p| p.first())
        .copied()
        .flatten()
        .ok_or(ParseError::InvalidField)?;
    let key = FunctionalKey::from_tilde_number(number)
        .map(Key::Functional)
        .unwrap_or(Key::Other(number));
    let (modifiers, event_type) = modifiers_and_type(fields.get(1))?;
    Ok(KeyEvent {
        key,
        shifted_key: None,
        base_layout_key: None,
        modifiers,
        event_type,
        associated_text: String::new(),
    })
}

fn parse_letter_sequence(params: &[u8], letter: u8) -> Result<KeyEvent, ParseError> {
    let key = Key::Functional(FunctionalKey::from_letter(letter).expect("pre-filtered"));
    let fields = if params.is_empty() {
        Vec::new()
    } else {
        split_params(params)?
    };

    // CSI 1;5A -> modifiers in field 1. CSI 5A -> the sole field is modifiers
    // unless it is the conventional leading `1`. CSI Z is legacy Shift+Tab.
    let modifiers_field = if letter == b'Z' && fields.len() <= 1 {
        fields.first()
    } else if fields.len() == 1 {
        let first = fields[0].first().copied().flatten();
        if first == Some(1) {
            None
        } else {
            fields.first()
        }
    } else {
        fields.get(1)
    };

    let (mut modifiers, event_type) = modifiers_and_type(modifiers_field)?;
    if letter == b'Z' {
        modifiers |= Modifiers::SHIFT;
    }

    Ok(KeyEvent {
        key,
        shifted_key: None,
        base_layout_key: None,
        modifiers,
        event_type,
        associated_text: String::new(),
    })
}

fn modifiers_and_type(
    field: Option<&Vec<Option<u32>>>,
) -> Result<(Modifiers, EventType), ParseError> {
    let Some(field) = field else {
        return Ok((Modifiers::NONE, EventType::Press));
    };
    let modifier_value = field.first().copied().flatten().unwrap_or(1);
    if modifier_value == 0 {
        return Err(ParseError::InvalidField);
    }
    let modifiers = Modifiers::from_protocol(modifier_value);
    let event_type = match field.get(1).copied().flatten() {
        None => EventType::Press,
        Some(code) => EventType::from_code(code).ok_or(ParseError::InvalidField)?,
    };
    Ok((modifiers, event_type))
}

fn associated_text(field: Option<&Vec<Option<u32>>>) -> Result<String, ParseError> {
    let Some(field) = field else {
        return Ok(String::new());
    };
    let mut text = String::new();
    for code in field {
        let Some(code) = *code else {
            continue;
        };
        if code < 0x20 || (0x80..0xa0).contains(&code) {
            return Err(ParseError::InvalidField);
        }
        let ch = char::from_u32(code).ok_or(ParseError::InvalidField)?;
        text.push(ch);
    }
    Ok(text)
}

/// Split CSI parameter bytes into `;`-separated fields of `:`-separated optional numbers.
fn split_params(params: &[u8]) -> Result<Vec<Vec<Option<u32>>>, ParseError> {
    if params.is_empty() {
        return Ok(Vec::new());
    }
    let mut fields = Vec::new();
    let mut field = Vec::new();
    let mut num: Option<u32> = None;
    let mut saw_digit = false;

    let push_sub = |field: &mut Vec<Option<u32>>, num: &mut Option<u32>, saw_digit: &mut bool| {
        if *saw_digit {
            field.push(*num);
        } else {
            field.push(None);
        }
        *num = None;
        *saw_digit = false;
    };

    for &b in params {
        match b {
            b'0'..=b'9' => {
                let digit = (b - b'0') as u32;
                let next = num
                    .unwrap_or(0)
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(digit));
                num = Some(next.ok_or(ParseError::InvalidParams)?);
                saw_digit = true;
            }
            b':' => {
                push_sub(&mut field, &mut num, &mut saw_digit);
            }
            b';' => {
                push_sub(&mut field, &mut num, &mut saw_digit);
                fields.push(std::mem::take(&mut field));
            }
            _ => return Err(ParseError::InvalidParams),
        }
    }
    push_sub(&mut field, &mut num, &mut saw_digit);
    fields.push(field);
    Ok(fields)
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csi(body: &str) -> Vec<u8> {
        let mut out = vec![ESC, b'['];
        out.extend_from_slice(body.as_bytes());
        out
    }

    fn key(bytes: &[u8]) -> KeyEvent {
        match parse_sequence(bytes).unwrap() {
            Event::Key(event) => event,
            other => panic!("expected key event, got {other:?}"),
        }
    }

    fn key_str(body: &str) -> KeyEvent {
        key(&csi(body))
    }

    #[test]
    fn plain_a() {
        let event = key_str("97u");
        assert_eq!(event.key, Key::Char('a'));
        assert_eq!(event.modifiers, Modifiers::NONE);
        assert_eq!(event.event_type, EventType::Press);
        assert!(event.associated_text.is_empty());
    }

    #[test]
    fn ctrl_a() {
        let event = key_str("97;5u");
        assert_eq!(event.key, Key::Char('a'));
        assert_eq!(event.modifiers, Modifiers::CTRL);
        assert!(event.is_action());
    }

    #[test]
    fn ctrl_shift_a() {
        let event = key_str("97;6u");
        assert_eq!(event.key, Key::Char('a'));
        assert!(event.modifiers.contains(Modifiers::SHIFT));
        assert!(event.modifiers.contains(Modifiers::CTRL));
    }

    #[test]
    fn release_a() {
        let event = key_str("97;1:3u");
        assert_eq!(event.event_type, EventType::Release);
        assert!(!event.is_action());
    }

    #[test]
    fn repeat_a() {
        let event = key_str("97;1:2u");
        assert_eq!(event.event_type, EventType::Repeat);
        assert!(event.is_action());
    }

    #[test]
    fn shifted_and_base_layout() {
        let event = key_str("97:65:113;2u");
        assert_eq!(event.key, Key::Char('a'));
        assert_eq!(event.shifted_key, Some('A'));
        assert_eq!(event.base_layout_key, Some('q'));
        assert_eq!(event.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn base_layout_without_shifted() {
        let event = key_str("97::99;5u");
        assert_eq!(event.key, Key::Char('a'));
        assert_eq!(event.shifted_key, None);
        assert_eq!(event.base_layout_key, Some('c'));
        assert_eq!(event.modifiers, Modifiers::CTRL);
    }

    #[test]
    fn associated_text() {
        let event = key_str("97;2;65u");
        assert_eq!(event.key, Key::Char('a'));
        assert_eq!(event.associated_text, "A");
    }

    #[test]
    fn unidentified_text_event() {
        let event = key_str("0;;229u");
        assert_eq!(event.key, Key::Unidentified);
        assert_eq!(event.associated_text, "å");
    }

    #[test]
    fn escape_enter_tab_backspace() {
        assert_eq!(key_str("27u").key, Key::Functional(FunctionalKey::Escape));
        assert_eq!(key_str("13u").key, Key::Functional(FunctionalKey::Enter));
        assert_eq!(key_str("9u").key, Key::Functional(FunctionalKey::Tab));
        assert_eq!(
            key_str("127u").key,
            Key::Functional(FunctionalKey::Backspace)
        );
    }

    #[test]
    fn arrow_unmodified() {
        assert_eq!(key_str("A").key, Key::Functional(FunctionalKey::Up));
        assert_eq!(key_str("B").key, Key::Functional(FunctionalKey::Down));
        assert_eq!(key_str("C").key, Key::Functional(FunctionalKey::Right));
        assert_eq!(key_str("D").key, Key::Functional(FunctionalKey::Left));
    }

    #[test]
    fn shift_up() {
        let event = key_str("1;2A");
        assert_eq!(event.key, Key::Functional(FunctionalKey::Up));
        assert_eq!(event.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn ctrl_up_omits_leading_one() {
        let event = key_str("5A");
        assert_eq!(event.key, Key::Functional(FunctionalKey::Up));
        assert_eq!(event.modifiers, Modifiers::CTRL);
    }

    #[test]
    fn f1_letter_and_tilde() {
        assert_eq!(key_str("P").key, Key::Functional(FunctionalKey::F(1)));
        assert_eq!(key_str("11~").key, Key::Functional(FunctionalKey::F(1)));
        assert_eq!(key_str("11;9~").modifiers, Modifiers::SUPER);
    }

    #[test]
    fn f5_through_f12() {
        assert_eq!(key_str("15~").key, Key::Functional(FunctionalKey::F(5)));
        assert_eq!(key_str("24~").key, Key::Functional(FunctionalKey::F(12)));
    }

    #[test]
    fn insert_delete_pages() {
        assert_eq!(key_str("2~").key, Key::Functional(FunctionalKey::Insert));
        assert_eq!(key_str("3~").key, Key::Functional(FunctionalKey::Delete));
        assert_eq!(key_str("5~").key, Key::Functional(FunctionalKey::PageUp));
        assert_eq!(key_str("6~").key, Key::Functional(FunctionalKey::PageDown));
    }

    #[test]
    fn keypad_enter_ctrl() {
        let event = key_str("57414;5u");
        assert_eq!(event.key, Key::Functional(FunctionalKey::KpEnter));
        assert_eq!(event.modifiers, Modifiers::CTRL);
    }

    #[test]
    fn left_shift_press() {
        let event = key_str("57441;2:1u");
        assert_eq!(event.key, Key::Functional(FunctionalKey::LeftShift));
        assert!(event.modifiers.contains(Modifiers::SHIFT));
        assert_eq!(event.event_type, EventType::Press);
    }

    #[test]
    fn ss3_arrow() {
        let event = key(&[ESC, b'O', b'A']);
        assert_eq!(event.key, Key::Functional(FunctionalKey::Up));
    }

    #[test]
    fn shift_tab_legacy() {
        let event = key_str("Z");
        assert_eq!(event.key, Key::Functional(FunctionalKey::Tab));
        assert!(event.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn modified_shift_tab_and_arrow_release() {
        let tab = key_str("1;5Z");
        assert_eq!(tab.key, Key::Functional(FunctionalKey::Tab));
        assert!(tab.modifiers.contains(Modifiers::SHIFT));
        assert!(tab.modifiers.contains(Modifiers::CTRL));

        let up = key_str("1;2:3A");
        assert_eq!(up.key, Key::Functional(FunctionalKey::Up));
        assert_eq!(up.event_type, EventType::Release);
        assert_eq!(up.modifiers, Modifiers::SHIFT);
    }

    #[test]
    fn flags_report() {
        match parse_sequence(&csi("?15u")).unwrap() {
            Event::FlagsReport(15) => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn incomplete_csi() {
        assert_eq!(parse(&[ESC]).unwrap(), Parse::Incomplete);
        assert_eq!(parse(&[ESC, b'[']).unwrap(), Parse::Incomplete);
        assert_eq!(parse(&csi("97")).unwrap(), Parse::Incomplete);
    }

    #[test]
    fn decoder_skips_plain_text_then_reads_key() {
        let mut decoder = Decoder::new();
        decoder.feed(b"hi");
        decoder.feed(&csi("97;5u"));
        assert_eq!(
            decoder.decode().unwrap(),
            Some(Event::Key(KeyEvent {
                key: Key::Char('a'),
                shifted_key: None,
                base_layout_key: None,
                modifiers: Modifiers::CTRL,
                event_type: EventType::Press,
                associated_text: String::new(),
            }))
        );
        assert_eq!(decoder.decode().unwrap(), None);
    }

    #[test]
    fn decoder_splits_across_feeds() {
        let mut decoder = Decoder::new();
        decoder.feed(&[ESC, b'[', b'9']);
        assert_eq!(decoder.decode().unwrap(), None);
        decoder.feed(b"7u");
        let Event::Key(event) = decoder.decode().unwrap().unwrap() else {
            panic!("expected key");
        };
        assert_eq!(event.key, Key::Char('a'));
    }

    #[test]
    fn mouse_sgr_is_skipped() {
        let mut seq = csi("<0;1;1M");
        seq.extend_from_slice(&csi("97u"));
        let mut decoder = Decoder::new();
        decoder.feed(&seq);
        let Event::Key(event) = decoder.decode().unwrap().unwrap() else {
            panic!("expected key");
        };
        assert_eq!(event.key, Key::Char('a'));
    }

    #[test]
    fn f13_pua() {
        assert_eq!(key_str("57376u").key, Key::Functional(FunctionalKey::F(13)));
        assert_eq!(key_str("57398u").key, Key::Functional(FunctionalKey::F(35)));
    }

    #[test]
    fn super_hyper_meta_caps_num() {
        // protocol field = 1 + 0b11111111
        let event = key_str("97;256u");
        assert!(event.modifiers.contains(Modifiers::SHIFT));
        assert!(event.modifiers.contains(Modifiers::ALT));
        assert!(event.modifiers.contains(Modifiers::CTRL));
        assert!(event.modifiers.contains(Modifiers::SUPER));
        assert!(event.modifiers.contains(Modifiers::HYPER));
        assert!(event.modifiers.contains(Modifiers::META));
        assert!(event.modifiers.contains(Modifiers::CAPS_LOCK));
        assert!(event.modifiers.contains(Modifiers::NUM_LOCK));
    }
}
