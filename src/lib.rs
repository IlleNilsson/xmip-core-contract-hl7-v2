#![forbid(unsafe_code)]

//! The HL7 v2 content contract — a technology of `xmip-core-contract`.
//!
//! Two claims, decided 2026-09-07 (ADR-0042): **well-formedness is a given**
//! and **conformance is a given once a contract is named**.
//!
//! Well-formed here is a *sound ER7 message*: it opens with `MSH`, MSH-1 is
//! the field separator and MSH-2 the four encoding characters, every segment
//! starts with a three-character identifier and ends at a carriage return,
//! and MSH-9 names a message type. Conformance is the message type: a Location
//! that names this contract with `ADT^A01` bound has every message's MSH-9
//! held to it, and `ADT^A01:2.5` holds MSH-12 as well. Every HL7 v2 version is
//! a version of this one technology and lives here.
//!
//! An HL7 message is answered, and the answer is an HL7 message; [`acknowledge`]
//! composes it from the message it answers, which is what a Receive Location
//! over `mllp` writes back on the connection.

use sdk::contract::{
    Contract, ContractDescriptor, ContractError, ContractFactory, ContractId, ValidationIssue,
    ValidationResult,
};
use stream::Stream;

/// The bound message type: `ADT^A01`, optionally with a version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageType {
    pub code: String,
    pub event: String,
    pub version: Option<String>,
}

impl MessageType {
    /// `ADT^A01` or `ADT^A01:2.5`.
    ///
    /// # Errors
    /// Not `CODE^EVENT`, with an optional `:version`.
    pub fn parse(reference: &str) -> Result<Self, ContractError> {
        let (kind, version) = reference
            .split_once(':')
            .map_or((reference, None), |(k, v)| (k, Some(v.trim().to_string())));
        match kind.trim().split_once('^') {
            Some((code, event)) if !code.is_empty() && !event.is_empty() => Ok(Self {
                code: code.to_ascii_uppercase(),
                event: event.to_ascii_uppercase(),
                version,
            }),
            _ => Err(ContractError {
                message: format!("{reference:?} is not CODE^EVENT or CODE^EVENT:version"),
            }),
        }
    }

    fn reference(&self) -> String {
        match &self.version {
            Some(version) => format!("{}^{}:{version}", self.code, self.event),
            None => format!("{}^{}", self.code, self.event),
        }
    }
}

/// The encoding characters a message declares in MSH-1 and MSH-2.
#[derive(Clone, Copy, Debug)]
pub struct Encoding {
    pub field: char,
    pub component: char,
    pub repetition: char,
    pub escape: char,
    pub subcomponent: char,
}

/// A message read into segments and fields, enough to judge and to answer.
pub struct Message<'a> {
    pub encoding: Encoding,
    /// Each segment's fields, the identifier at index 0. For `MSH` field 1 is
    /// the separator itself, so `MSH-9` is index 9 there as everywhere.
    pub segments: Vec<Vec<&'a str>>,
}

impl<'a> Message<'a> {
    /// Read `text` as ER7.
    ///
    /// # Errors
    /// The one issue that makes it not an HL7 message.
    pub fn parse(text: &'a str) -> Result<Self, ValidationIssue> {
        let text = text.trim_end_matches(['\r', '\n']);
        let mut chars = text.chars();
        let header: String = chars.by_ref().take(3).collect();
        if header != "MSH" {
            return Err(malformed("the message does not start with MSH", "MSH"));
        }
        let mut encoding_chars = chars.take(5);
        let field = encoding_chars
            .next()
            .ok_or_else(|| malformed("MSH-1 has no field separator", "MSH-1"))?;
        let rest: Vec<char> = encoding_chars.collect();
        if rest.len() < 4 || rest.contains(&field) {
            return Err(malformed("MSH-2 is not four encoding characters", "MSH-2"));
        }
        let encoding = Encoding {
            field,
            component: rest[0],
            repetition: rest[1],
            escape: rest[2],
            subcomponent: rest[3],
        };
        let mut segments = Vec::new();
        for (ordinal, line) in text
            .split(['\r', '\n'])
            .filter(|l| !l.is_empty())
            .enumerate()
        {
            let mut fields: Vec<&str> = line.split(field).collect();
            if fields[0].len() != 3 || !fields[0].bytes().all(|b| b.is_ascii_alphanumeric()) {
                let at = format!("segment {}", ordinal + 1);
                return Err(malformed(
                    &format!("{:?} is not a segment identifier", fields[0]),
                    &at,
                ));
            }
            if fields[0] == "MSH" {
                // MSH-1 is the separator the split consumed; put it back so
                // MSH-n is index n like every other segment.
                fields.insert(1, &line[3..4]);
            }
            segments.push(fields);
        }
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

/// A `malformed` issue placed at `path`: ER7 can always say which segment or
/// field stopped it being a message, so the capability's unplaced one is not
/// used here.
fn malformed(message: &str, path: &str) -> ValidationIssue {
    ValidationIssue::at("malformed", message, path)
}

/// The HL7 v2 contract, bare or bound to a message type.
pub struct Hl7v2 {
    descriptor: ContractDescriptor,
    message_type: Option<MessageType>,
}

impl Hl7v2 {
    /// A sound message of any type.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: descriptor("hl7-v2"),
            message_type: None,
        }
    }

    /// A sound message of `message_type`.
    #[must_use]
    pub fn of(message_type: MessageType) -> Self {
        Self {
            descriptor: descriptor(&format!("hl7-v2:{}", message_type.reference())),
            message_type: Some(message_type),
        }
    }
}

impl Default for Hl7v2 {
    fn default() -> Self {
        Self::new()
    }
}

fn descriptor(id: &str) -> ContractDescriptor {
    ContractDescriptor {
        id: ContractId(id.to_string()),
        version: "1".to_string(),
        representation: "x-application/hl7-v2+er7".to_string(),
    }
}

impl Contract for Hl7v2 {
    fn descriptor(&self) -> &ContractDescriptor {
        &self.descriptor
    }

    fn identify(&self, stream: &Stream) -> Result<bool, ContractError> {
        Ok(stream.bytes().starts_with(b"MSH"))
    }

    fn validate(&self, stream: &Stream) -> Result<ValidationResult, ContractError> {
        let mut issues = Vec::new();
        let text = match std::str::from_utf8(stream.bytes()) {
            Ok(text) => text,
            Err(error) => {
                return Ok(ValidationResult::of(vec![malformed(
                    &format!("not text: {error}"),
                    "",
                )]));
            }
        };
        let message = match Message::parse(text) {
            Ok(message) => message,
            Err(issue) => return Ok(ValidationResult::of(vec![issue])),
        };
        let (code, event) = message.message_type();
        if code.is_empty() {
            issues.push(malformed("MSH-9 names no message type", "MSH-9"));
        }
        if let Some(wanted) = &self.message_type {
            if code != wanted.code || event != wanted.event {
                let message = format!(
                    "is {code}^{event}, the contract is {}^{}",
                    wanted.code, wanted.event
                );
                issues.push(ValidationIssue::at("message-type", &message, "MSH-9"));
            }
            if let Some(version) = &wanted.version
                && message.field("MSH", 12) != version
            {
                let actual = message.field("MSH", 12);
                let message = format!("is {actual:?}, the contract is {version}");
                issues.push(ValidationIssue::at("version", &message, "MSH-12"));
            }
        }
        Ok(ValidationResult::of(issues))
    }
}

/// What an acknowledgement says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acknowledgement {
    /// Application accept.
    Accept,
    /// Application error: the message was understood and refused.
    Error,
    /// Application reject: the message could not be processed.
    Reject,
}

/// The `ACK` for `message`: MSH with sender and receiver swapped, the same
/// control id echoed in MSA-2, and `text` as MSA-3 when there is something to
/// say. `original` is the text of the message answered.
#[must_use]
pub fn acknowledge(original: &str, verdict: Acknowledgement, text: &str) -> String {
    let code = match verdict {
        Acknowledgement::Accept => "AA",
        Acknowledgement::Error => "AE",
        Acknowledgement::Reject => "AR",
    };
    let Ok(message) = Message::parse(original) else {
        return format!("MSH|^~\\&|||||||ACK|1|P|2.5\rMSA|{code}||{text}\r");
    };
    let e = message.encoding;
    let f = e.field;
    let (_, event) = message.message_type();
    let version = message.field("MSH", 12);
    format!(
        "MSH{f}{}{}{}{}{f}{}{f}{}{f}{}{f}{}{f}{}{f}{f}ACK{}{event}{f}{}{f}{}{f}{}\r\
         MSA{f}{code}{f}{}{f}{text}\r",
        e.component,
        e.repetition,
        e.escape,
        e.subcomponent,
        message.field("MSH", 5),
        message.field("MSH", 6),
        message.field("MSH", 3),
        message.field("MSH", 4),
        message.field("MSH", 7),
        e.component,
        message.field("MSH", 10),
        message.field("MSH", 11),
        if version.is_empty() { "2.5" } else { version },
        message.field("MSH", 10),
    )
}

/// Loads the contract a Location names: an empty reference is the bare
/// contract, anything else a message type, `ADT^A01` or `ADT^A01:2.5`.
pub struct Hl7v2Factory;

impl ContractFactory for Hl7v2Factory {
    fn technology(&self) -> &'static str {
        "hl7-v2"
    }

    fn load(&self, reference: &str) -> Result<Box<dyn Contract>, ContractError> {
        if reference.trim().is_empty() {
            return Ok(Box::new(Hl7v2::new()));
        }
        Ok(Box::new(Hl7v2::of(MessageType::parse(reference)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::fixture::stream;

    const ADT: &str = "MSH|^~\\&|LAB|HOSP|EMR|CLINIC|20260908103000||ADT^A01|MSG0001|P|2.5\r\
PID|1||123456^^^HOSP^MR||DOE^JOHN||19800101|M\rPV1|1|I|WARD^101^A\r";

    #[test]
    fn a_sound_message_holds_bare_and_reads_its_type() {
        let held = Hl7v2::new().validate(&stream(ADT)).expect("validates");
        assert!(held.valid, "issues: {:?}", held.issues);
        let message = Message::parse(ADT).expect("parses");
        assert_eq!(
            message.message_type(),
            ("ADT".to_string(), "A01".to_string())
        );
        assert_eq!(message.field("PID", 5), "DOE^JOHN");
        assert_eq!(message.field("MSH", 12), "2.5");
    }

    #[test]
    fn what_is_not_hl7_is_named() {
        let cases = [
            ("<xml/>", "MSH"),
            ("MSH|^~\\&|A\rBAD SEGMENT|x\r", "segment 2"),
            ("MSH|^~|&|A\r", "MSH-2"),
        ];
        for (text, path) in cases {
            let held = Hl7v2::new().validate(&stream(text)).expect("validates");
            assert_eq!(held.issues[0].code, "malformed", "{text}");
            assert_eq!(held.issues[0].path.as_deref(), Some(path), "{text}");
        }
    }

    #[test]
    fn a_bound_type_holds_and_names_the_departure() {
        let bound = Hl7v2::of(MessageType::parse("ADT^A01:2.5").expect("type"));
        assert_eq!(bound.descriptor().id.0, "hl7-v2:ADT^A01:2.5");
        assert!(bound.validate(&stream(ADT)).expect("validates").valid);
        let oru = Hl7v2::of(MessageType::parse("oru^r01").expect("type"));
        let held = oru.validate(&stream(ADT)).expect("validates");
        assert_eq!(held.issues[0].code, "message-type");
        let v23 = Hl7v2::of(MessageType::parse("ADT^A01:2.3").expect("type"));
        let held = v23.validate(&stream(ADT)).expect("validates");
        assert_eq!(held.issues[0].code, "version");
        assert!(MessageType::parse("ADT").is_err());
    }

    #[test]
    fn the_acknowledgement_answers_the_message_it_was_given() {
        let ack = acknowledge(ADT, Acknowledgement::Accept, "");
        let message = Message::parse(&ack).expect("parses");
        assert_eq!(message.field("MSH", 3), "EMR");
        assert_eq!(message.field("MSH", 5), "LAB");
        assert_eq!(message.field("MSH", 9), "ACK^A01");
        assert_eq!(message.field("MSA", 1), "AA");
        assert_eq!(message.field("MSA", 2), "MSG0001");
        assert!(
            Hl7v2::new()
                .validate(&stream(&ack))
                .expect("validates")
                .valid
        );
        let refused = acknowledge(ADT, Acknowledgement::Error, "PID-5 missing");
        let message = Message::parse(&refused).expect("parses");
        assert_eq!(message.field("MSA", 1), "AE");
        assert_eq!(message.field("MSA", 3), "PID-5 missing");
    }

    #[test]
    fn the_factory_reads_the_type_off_the_reference() {
        let factory = Hl7v2Factory;
        assert_eq!(factory.technology(), "hl7-v2");
        assert_eq!(factory.load("").expect("bare").descriptor().id.0, "hl7-v2");
        assert_eq!(
            factory.load("ORU^R01").expect("typed").descriptor().id.0,
            "hl7-v2:ORU^R01"
        );
        assert!(factory.load("nonsense").is_err());
    }
}
