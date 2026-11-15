//! An HL7 v2 message in ER7, read: the encoding characters its `MSH`
//! declares, and its segments walked one at a time by the Foundation's one
//! segment walk (`message::segment`), which the ER7 shape sections by too:
//! every line break ends a segment, an empty line is none, nothing but line
//! breaks stands between two, and the last segment ends where the text
//! does. What reads one field cuts only its
//! segment and walks the rest uncut ([`Message::walk`]), and
//! [`Message::parse`] cuts every segment into its fields: one walk, so a
//! message is refused in the same words, at the same segment, by both.

use contract::ValidationIssue;
use message::segment::{Delimiters, Walk};

use crate::malformed;

/// The encoding characters a message declares in MSH-1 and MSH-2.
#[derive(Clone, Copy, Debug)]
pub struct Encoding {
    pub field: char,
    pub component: char,
    pub repetition: char,
    pub escape: char,
    pub subcomponent: char,
}

impl Encoding {
    /// What `text`'s `MSH` declares.
    ///
    /// # Errors
    /// The issue that makes it not an HL7 message: no `MSH` at its head, or
    /// MSH-1 or MSH-2 not ASCII and distinct.
    fn declared(text: &str) -> Result<Self, ValidationIssue> {
        let Some(after) = text.strip_prefix("MSH") else {
            return Err(malformed("the message does not start with MSH", "MSH"));
        };
        let mut encoding_chars = after.chars().take(5);
        let field = encoding_chars
            .next()
            .filter(char::is_ascii)
            .ok_or_else(|| malformed("MSH-1 is not one ASCII field separator", "MSH-1"))?;
        let rest: Vec<char> = encoding_chars.collect();
        if rest.len() < 4 || rest.contains(&field) || !rest.iter().all(char::is_ascii) {
            return Err(malformed("MSH-2 is not four ASCII characters", "MSH-2"));
        }
        Ok(Self {
            field,
            component: rest[0],
            repetition: rest[1],
            escape: rest[2],
            subcomponent: rest[3],
        })
    }

    /// What the walk cuts by: every line break ends a segment, and the last
    /// one ends where the text does. MSH-1 and MSH-2 are ASCII, as
    /// [`Encoding::declared`] holds them.
    fn delimiters(self) -> Delimiters {
        let byte = |ch: char| u8::try_from(ch).unwrap_or_default();
        Delimiters::new(b'\r', byte(self.field), byte(self.component))
            .ended_by_line_breaks()
            .open_at_the_end()
    }

    /// `text` as the value it spells: the escape sequences HL7 v2 writes for
    /// its own characters (`\F\`, `\S\`, `\T\`, `\R\`, `\E\`) read back,
    /// any other sequence left as written.
    #[must_use]
    pub fn unescape(&self, text: &str) -> String {
        let e = self.escape;
        if !text.contains(e) {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find(e) {
            out.push_str(&rest[..at]);
            let after = &rest[at + e.len_utf8()..];
            let mut sequence = after.chars();
            let read = match (sequence.next(), sequence.next()) {
                (Some('F'), Some(end)) if end == e => Some(self.field),
                (Some('S'), Some(end)) if end == e => Some(self.component),
                (Some('T'), Some(end)) if end == e => Some(self.subcomponent),
                (Some('R'), Some(end)) if end == e => Some(self.repetition),
                (Some('E'), Some(end)) if end == e => Some(e),
                _ => None,
            };
            if let Some(character) = read {
                out.push(character);
                rest = &after[1 + e.len_utf8()..];
            } else {
                out.push(e);
                rest = after;
            }
        }
        out.push_str(rest);
        out
    }

    /// `value` as a field writes it: every one of the encoding characters
    /// escaped, the reverse of [`Encoding::unescape`].
    #[must_use]
    pub fn escape(&self, value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        for character in value.chars() {
            let letter = match character {
                c if c == self.escape => Some('E'),
                c if c == self.field => Some('F'),
                c if c == self.component => Some('S'),
                c if c == self.subcomponent => Some('T'),
                c if c == self.repetition => Some('R'),
                _ => None,
            };
            match letter {
                Some(letter) => {
                    out.push(self.escape);
                    out.push(letter);
                    out.push(self.escape);
                }
                None => out.push(character),
            }
        }
        out
    }
}

/// A message read into segments and fields, enough to judge and to answer.
pub struct Message<'a> {
    pub encoding: Encoding,
    /// Each segment's fields, the identifier at index 0. For `MSH` field 1 is
    /// the separator itself, so `MSH-9` is index 9 there as everywhere.
    pub segments: Vec<Vec<&'a str>>,
}

impl<'a> Message<'a> {
    /// `text`'s encoding characters, and its segments one at a time, in
    /// the order they lie.
    ///
    /// # Errors
    /// The issue that makes its head not an HL7 message's.
    pub fn walk(text: &'a str) -> Result<(Encoding, Segments<'a>), ValidationIssue> {
        let text = text.trim_end_matches(['\r', '\n']);
        let encoding = Encoding::declared(text)?;
        let segments = Segments {
            text,
            field: encoding.field,
            walk: Some(encoding.delimiters().walk(text.as_bytes())),
            ordinal: 0,
        };
        Ok((encoding, segments))
    }

    /// Read `text` as ER7.
    ///
    /// # Errors
    /// The one issue that makes it not an HL7 message.
    pub fn parse(text: &'a str) -> Result<Self, ValidationIssue> {
        let (encoding, walk) = Self::walk(text)?;
        let segments = walk
            .map(|lying| lying.map(|lying| lying.fields()))
            .collect::<Result<_, _>>()?;
        Ok(Self { encoding, segments })
    }

    /// Field `index` of the first segment named `id`, or empty.
    #[must_use]
    pub fn field(&self, id: &str, index: usize) -> &'a str {
        self.segments
            .iter()
            .find(|s| s[0] == id)
            .and_then(|s| s.get(index).copied())
            .unwrap_or("")
    }

    /// MSH-9 as `CODE^EVENT`, its first two components.
    #[must_use]
    pub fn message_type(&self) -> (String, String) {
        let mut parts = self.field("MSH", 9).split(self.encoding.component);
        (
            parts.next().unwrap_or("").to_ascii_uppercase(),
            parts.next().unwrap_or("").to_ascii_uppercase(),
        )
    }
}

/// A message's segments, one at a time, in the order they lie, as
/// [`Message::walk`] begins them. After a refusal it yields nothing.
#[derive(Clone, Debug)]
pub struct Segments<'a> {
    text: &'a str,
    field: char,
    /// None once the walk has ended.
    walk: Option<Walk<'a>>,
    /// How many segments were walked, the one at hand included.
    ordinal: usize,
}

impl<'a> Iterator for Segments<'a> {
    type Item = Result<Lying<'a>, ValidationIssue>;

    fn next(&mut self) -> Option<Self::Item> {
        let walked = self.walk.as_mut()?.next()?;
        self.ordinal += 1;
        let at = || format!("segment {}", self.ordinal);
        let lying = walked
            .map_err(|(reason, _)| malformed(reason, &at()))
            .and_then(|walked| {
                let text = &self.text[walked.offset..walked.offset + walked.bytes.len()];
                Lying::cut(text, walked.tag, self.field).map_err(|reason| malformed(&reason, &at()))
            });
        if lying.is_err() {
            self.walk = None;
        }
        Some(lying)
    }
}

/// One segment as it lies in the message: its text, cut into fields only
/// when asked.
#[derive(Clone, Copy, Debug)]
pub struct Lying<'a> {
    text: &'a str,
    field: char,
}

impl<'a> Lying<'a> {
    /// `text`, one segment, its identifier — what stands before its first
    /// field separator — held to three letters or digits, and an `MSH`'s to
    /// its separator. `tagged` is the run of letters and digits the walk
    /// read it opening with: the identifier where the separator or the end
    /// follows it.
    fn cut(text: &'a str, tagged: &str, field: char) -> Result<Self, String> {
        let after = text.as_bytes().get(3).map(|&byte| char::from(byte));
        if tagged.len() != 3 || after.is_some_and(|after| after != field) {
            let identifier = text.split(field).next().unwrap_or_default();
            return Err(format!("{identifier:?} is not a segment identifier"));
        }
        // MSH-1 is the separator after the identifier, ASCII.
        if tagged == "MSH" && after.is_none() {
            return Err("an MSH segment lacks its separator".to_string());
        }
        Ok(Self { text, field })
    }

    /// The segment's identifier: `MSH`, `PID`, `OBX`.
    #[must_use]
    pub fn identifier(&self) -> &'a str {
        let text: &'a str = self.text;
        &text[..3]
    }

    /// The segment's fields as written, the identifier at index 0. For
    /// `MSH` field 1 is the separator itself, put back where it stands, so
    /// `MSH-9` is index 9 there as everywhere.
    #[must_use]
    pub fn fields(&self) -> Vec<&'a str> {
        let text: &'a str = self.text;
        let mut fields: Vec<&'a str> = text.split(self.field).collect();
        if self.identifier() == "MSH" {
            fields.insert(1, &text[3..4]);
        }
        fields
    }

    /// Field `n` as [`Lying::fields`] counts it, cutting no other.
    #[must_use]
    pub fn field(&self, n: usize) -> Option<&'a str> {
        let text: &'a str = self.text;
        let msh = self.identifier() == "MSH";
        match n {
            1 if msh => Some(&text[3..4]),
            n if msh && n > 1 => text.split(self.field).nth(n - 1),
            n => text.split(self.field).nth(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADT: &str = "MSH|^~\\&|LAB|HOSP|EMR|CLINIC|20260908103000||ADT^A01|MSG0001|P|2.5\r\
PID|1||123456^^^HOSP^MR||DOE^JOHN||19800101|M\rPV1|1|I|WARD^101^A\r";

    #[test]
    fn a_walk_yields_a_segment_at_a_time_and_cuts_one_field_only_when_asked() {
        let (encoding, mut walk) = Message::walk(ADT).expect("declared");
        assert_eq!(encoding.component, '^');
        let msh = walk.next().expect("MSH").expect("sound");
        assert_eq!(msh.identifier(), "MSH");
        assert_eq!(msh.field(1), Some("|"));
        assert_eq!(msh.field(2), Some("^~\\&"));
        assert_eq!(msh.field(9), Some("ADT^A01"));
        assert_eq!(msh.field(0), Some("MSH"));
        let pid = walk.next().expect("PID").expect("sound");
        assert_eq!(pid.field(5), Some("DOE^JOHN"));
        assert_eq!(pid.field(30), None);
        assert_eq!(
            walk.next().expect("PV1").expect("sound").identifier(),
            "PV1"
        );
        assert!(walk.next().is_none());
        // Every field the walk cuts is the one the whole message holds.
        let message = Message::parse(ADT).expect("parses");
        let (_, walk) = Message::walk(ADT).expect("declared");
        for (lying, fields) in walk.zip(&message.segments) {
            let lying = lying.expect("sound");
            for (n, field) in fields.iter().enumerate() {
                assert_eq!(lying.field(n), Some(*field), "{}-{n}", fields[0]);
            }
            assert_eq!(lying.field(fields.len()), None);
        }
    }

    #[test]
    fn every_line_break_ends_a_segment_and_the_last_may_end_with_the_text() {
        let lines = "MSH|^~\\&|A|B\r\n\nPID|1\nPV1|1";
        let message = Message::parse(lines).expect("parses");
        let identifiers: Vec<&str> = message.segments.iter().map(|s| s[0]).collect();
        assert_eq!(identifiers, ["MSH", "PID", "PV1"]);
        assert_eq!(message.field("PV1", 1), "1");
    }

    #[test]
    fn a_walk_refuses_at_the_segment_it_reaches_and_ends_there() {
        let (_, mut walk) =
            Message::walk("MSH|^~\\&|A\rPID|1\rBAD SEGMENT|x\rPV1|1\r").expect("declared");
        assert!(walk.next().expect("MSH").is_ok());
        assert!(walk.next().expect("PID").is_ok());
        let refused = walk.next().expect("bad").expect_err("refused");
        assert_eq!(
            refused.message,
            "\"BAD SEGMENT\" is not a segment identifier"
        );
        assert_eq!(refused.path.as_deref(), Some("segment 3"));
        assert!(walk.next().is_none(), "a walk ends where it refuses");
        // Only line breaks stand between two segments.
        let spaced = Message::parse("MSH|^~\\&|A\r PID|1\r")
            .err()
            .expect("spaced");
        assert_eq!(spaced.message, "\" PID\" is not a segment identifier");
        assert_eq!(spaced.path.as_deref(), Some("segment 2"));
    }
}
