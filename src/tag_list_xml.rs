use crate::encoding::{ByteOrder, Encoding, ValueType, WordOrder};
use crate::presentation::Presentation;
use crate::tag_list::{
    Bit, IntegerEnum, RegisterField, RegisterGroup, RegisterOrGroup, RegisterRange, TagList,
};
use roxmltree::{Node, TextPos};
use std::error::Error;
use std::num::ParseIntError;
use std::str::{FromStr, ParseBoolError};

const NS: &str = "http://www.elektro-kapsel.se/xml/modbus_config/v1";

#[derive(Debug)]
pub struct ParseError {
    kind: ParseErrorKind,
    pos: TextPos,
}

impl ParseError {
    pub fn new(node: &Node, kind: ParseErrorKind) -> ParseError {
        ParseError {
            pos: node.document().text_pos_at(node.range().start),
            kind,
        }
    }
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{}:{}: {}", self.pos.row, self.pos.col, self.kind)
    }
}

use ParseErrorKind::*;

#[derive(Debug)]
pub enum ParseErrorKind {
    WrongNamespace,
    UnexpectedElement,
    UnexpectedText,
    UnexpectedAttribute,
    MissingAttribute(String),
    ParseAttribute(String, Box<dyn Error + Send + Sync>),
    BitRange,
    InvalidByteOrder,
    InvalidWordOrder,
    InvalidSign,
    InvalidValueType,
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            WrongNamespace => write!(f, "Incorrect namespace for element"),
            UnexpectedElement => write!(f, "Unexpected element"),
            UnexpectedText => write!(f, "Unexpected non-whitespace text"),
            UnexpectedAttribute => write!(f, "Unexpected attribute"),
            MissingAttribute(name) => write!(f, "Missing attribute '{name}'"),
            ParseAttribute(name, err) => write!(f, "Failed to parse attribute '{name}': {err}"),
            BitRange => write!(
                f,
                "Either use attribute 'bit' or both of 'bit-low' and 'bit-high'"
            ),
            InvalidByteOrder => write!(f, "Invalid byte order"),
            InvalidWordOrder => write!(f, "Invalid word order"),
            InvalidSign => write!(f, "Attribute 'sign' must be either 'signed' or 'unsigned'"),
            InvalidValueType => write!(
                f,
                "Attribute 'value-type' must be one of 'integer', 'float', or 'string'"
            ),
        }
    }
}

fn check_element_ns(node: &Node) -> Result<bool, ParseError> {
    if node.is_element() {
        if node.tag_name().namespace() != Some(NS) {
            return Err(ParseError::new(node, WrongNamespace));
        }
        return Ok(true);
    } else if node.is_text() {
        if let Some(text) = node.text() {
            // Don't allow non-whitespace around elements
            if text.find(|c: char| !c.is_whitespace()).is_some() {
                return Err(ParseError::new(node, UnexpectedText));
            }
        }
    }
    Ok(false)
}

fn required_attribute<T>(node: &Node, name: &str) -> Result<T, ParseError>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let attr_str = node
        .attribute(name)
        .ok_or_else(|| ParseError::new(node, MissingAttribute(name.to_string())))?;
    let res: Result<T, <T as FromStr>::Err> = attr_str.parse();
    res.map_err(|e| ParseError::new(node, ParseAttribute(name.to_string(), e.into())))
}

fn optional_attribute<T>(node: &Node, name: &str) -> Result<Option<T>, ParseError>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let attr_str = match node.attribute(name) {
        Some(v) => v,
        None => return Ok(None),
    };
    let res: Result<T, <T as FromStr>::Err> = attr_str.parse();
    match res {
        Ok(res) => Ok(Some(res)),
        Err(e) => Err(ParseError::new(
            node,
            ParseAttribute(name.to_string(), e.into()),
        )),
    }
}

pub fn parse_presentation(node: &Node) -> Result<Presentation, ParseError> {
    let scale: f32 = optional_attribute(node, "scale")?.unwrap_or(1.0);
    let unit: Option<String> = optional_attribute(node, "unit")?;
    let radix = optional_attribute::<u8>(node, "radix")?.unwrap_or(10);
    let decimals = optional_attribute::<u8>(node, "decimals")?.unwrap_or(2);
    Ok(Presentation {
        decimals,
        radix,
        scale,
        unit,
    })
}

pub fn parse_encoding(node: &Node) -> Result<Encoding, ParseError> {
    let byte_order = match optional_attribute::<String>(node, "byte-order")? {
        Some(s) => {
            if s.starts_with("little") {
                ByteOrder::LittleEndian
            } else if s.starts_with("big") {
                ByteOrder::BigEndian
            } else {
                return Err(ParseError::new(node, ParseErrorKind::InvalidByteOrder));
            }
        }
        None => ByteOrder::BigEndian,
    };
    let word_order = match optional_attribute::<String>(node, "word-order")? {
        Some(s) => {
            if s.starts_with("little") {
                WordOrder::LittleEndian
            } else if s.starts_with("big") {
                WordOrder::BigEndian
            } else {
                return Err(ParseError::new(node, ParseErrorKind::InvalidByteOrder));
            }
        }
        None => WordOrder::BigEndian,
    };
    let value = match optional_attribute::<String>(node, "value-type")?
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| "integer")
    {
        "integer" => {
            let signed = match optional_attribute::<String>(node, "sign")?
                .as_ref()
                .map(|s| s.as_str())
            {
                Some("signed") => true,
                Some("unsigned") => false,
                Some(_) => return Err(ParseError::new(node, ParseErrorKind::InvalidSign)),
                None => false,
            };

            ValueType::Integer { signed }
        }
        "float" => ValueType::Float,
        "string" => {
            let fill: u8 = optional_attribute(node, "fill")?.unwrap_or(0);
            ValueType::String { fill }
        }
        _ => return Err(ParseError::new(node, ParseErrorKind::InvalidValueType)),
    };

    Ok(Encoding {
        value,
        byte_order,
        word_order,
    })
}

pub fn parse_enum(node: &Node) -> Result<IntegerEnum, ParseError> {
    let label: String = required_attribute(node, "label")?;
    let value = required_attribute::<ParsedU16>(node, "value")?.into();
    Ok(IntegerEnum { value, label })
}

pub fn parse_register_field(node: &Node) -> Result<RegisterField, ParseError> {
    let bit: Option<u8> = optional_attribute(node, "bit")?;
    let bit_low: Option<u8> = optional_attribute(node, "bit-low")?;
    let bit_high: Option<u8> = optional_attribute(node, "bit-high")?;
    let label: Option<String> = optional_attribute(node, "label")?;
    let (bit_low, bit_high) = match (bit, bit_low, bit_high) {
        (Some(bit), None, None) => (bit, bit),
        (None, Some(low), Some(high)) => (low, high),
        _ => return Err(ParseError::new(node, ParseErrorKind::BitRange)),
    };
    let presentation = parse_presentation(node)?;
    let mut enums = Vec::new();
    for child in node.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "enum" => {
                    let enu = parse_enum(&child)?;
                    enums.push(enu);
                }
                _ => return Err(ParseError::new(&child, UnexpectedElement)),
            }
        }
    }
    Ok(RegisterField {
        bit_low,
        bit_high,
        label,
        presentation,
        enums,
    })
}

pub fn parse_register(node: &Node) -> Result<RegisterRange, ParseError> {
    let address_low: u16;
    let address_high: u16;
    if node.tag_name().name() == "register" {
        address_low = required_attribute::<ParsedU16>(node, "addr")?.into();
        address_high = address_low;
    } else {
        address_low = required_attribute::<ParsedU16>(node, "addr-low")?.into();
        address_high = required_attribute::<ParsedU16>(node, "addr-high")?.into();
    }
    let label: Option<String> = optional_attribute(node, "label")?;
    let initial_value: Option<String> = optional_attribute(node, "initial-value")?;
    let presentation = parse_presentation(node)?;
    let encoding = parse_encoding(node)?;

    let mut fields = Vec::new();
    let mut enums = Vec::new();
    for child in node.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "field" => {
                    let field = parse_register_field(&child)?;
                    fields.push(field);
                }
                "enum" => {
                    let enu = parse_enum(&child)?;
                    enums.push(enu);
                }
                _ => return Err(ParseError::new(&child, UnexpectedElement)),
            }
        }
    }
    Ok(RegisterRange {
        address_low,
        address_high,
        label,
        fields,
        initial_value,
        presentation,
        encoding,
        enums,
    })
}

pub fn parse_group(node: &Node) -> Result<RegisterGroup, ParseError> {
    let base_address = required_attribute::<ParsedU16>(node, "base-addr")?.into();
    let label: Option<String> = optional_attribute(node, "label")?;
    let registers = parse_registers_or_groups(node)?;
    Ok(RegisterGroup {
        base_address,
        label,
        registers,
    })
}

pub fn parse_registers_or_groups(parent: &Node) -> Result<Vec<RegisterOrGroup>, ParseError> {
    let mut regs = Vec::new();
    for child in parent.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "register" => {
                    let reg = parse_register(&child)?;
                    regs.push(RegisterOrGroup::Register(reg));
                }
                "register-range" => {
                    let reg = parse_register(&child)?;
                    regs.push(RegisterOrGroup::Register(reg));
                }
                "group" => {
                    let reg = parse_group(&child)?;
                    regs.push(RegisterOrGroup::Group(reg));
                }

                _ => return Err(ParseError::new(&child, UnexpectedElement)),
            }
        }
    }
    Ok(regs)
}

struct ParsedBit(bool);
impl FromStr for ParsedBit {
    type Err = ParseBoolError;
    fn from_str(s: &str) -> Result<Self, ParseBoolError> {
        match s {
            "1" => Ok(ParsedBit(true)),
            "0" => Ok(ParsedBit(false)),
            _ => bool::from_str(s).map(ParsedBit),
        }
    }
}

impl From<ParsedBit> for bool {
    fn from(b: ParsedBit) -> bool {
        b.0
    }
}

struct ParsedU16(u16);
impl FromStr for ParsedU16 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, ParseIntError> {
        let (s, neg) = match s.strip_prefix('-') {
            Some(s) => (s, true),
            None => (s, false),
        };
        let pos_int = if let Some(s) = s.strip_prefix("0x") {
            u16::from_str_radix(s, 16)
        } else if let Some(s) = s.strip_prefix("0b") {
            u16::from_str_radix(s, 2)
        } else {
            str::parse(s)
        };
        pos_int
            .map(|i| if neg { (-(i as i16)) as u16 } else { i })
            .map(ParsedU16)
    }
}

impl From<ParsedU16> for u16 {
    fn from(b: ParsedU16) -> u16 {
        b.0
    }
}

pub fn parse_bit(node: &Node) -> Result<Bit, ParseError> {
    let address: u16 = required_attribute::<ParsedU16>(node, "addr")?.into();
    let label: Option<String> = optional_attribute(node, "label")?;
    let initial_value: Option<bool> =
        optional_attribute::<ParsedBit>(node, "initial-value")?.map(|b| b.into());

    Ok(Bit {
        address,
        label,
        initial_value,
    })
}

pub fn parse_bits(parent: &Node) -> Result<Vec<Bit>, ParseError> {
    let mut bits = Vec::new();
    for child in parent.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "bit" => {
                    let bit = parse_bit(&child)?;
                    bits.push(bit);
                }
                _ => return Err(ParseError::new(&child, UnexpectedElement)),
            }
        }
    }
    Ok(bits)
}

pub fn parse_tag_list(node: &Node) -> Result<TagList, ParseError> {
    let mut holding_registers = None;
    let mut input_registers = None;
    let mut discrete_inputs = None;
    let mut coils = None;
    for child in node.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "holding-registers" => {
                    let regs = parse_registers_or_groups(&child)?;
                    holding_registers = Some(regs);
                }
                "input-registers" => {
                    let regs = parse_registers_or_groups(&child)?;
                    input_registers = Some(regs);
                }
                "discrete-inputs" => {
                    let bits = parse_bits(&child)?;
                    discrete_inputs = Some(bits);
                }
                "coils" => {
                    let bits = parse_bits(&child)?;
                    coils = Some(bits);
                }
                _ => return Err(ParseError::new(&child, UnexpectedElement)),
            }
        }
    }
    Ok(TagList {
        holding_registers: holding_registers.unwrap_or_default(),
        input_registers: input_registers.unwrap_or_default(),
        discrete_inputs: discrete_inputs.unwrap_or_default(),
        coils: coils.unwrap_or_default(),
    })
}

#[cfg(test)]
use roxmltree::Document;

#[test]
fn parse_register_test() -> Result<(), ParseError> {
    let doc = Document::parse(
        r#"
    <holding-registers xmlns="http://www.elektro-kapsel.se/xml/modbus_config/v1">
    <register addr="0" label="Reg 0">
        <field bit="0" label="0.0" />
        <field bit-low="1" bit-high="8" label="0.1-8" />
    </register>
    <register addr="1" label="Reg 1" />
</holding-registers>
"#,
    )
    .unwrap();

    let regs = parse_registers(&doc.root().first_child().unwrap())?;
    assert_eq!(regs[0].address_low, 0);
    assert_eq!(regs[0].label, Some("Reg 0".to_string()));
    assert_eq!(regs[0].fields[0].bit_low, 0);
    assert_eq!(regs[0].fields[0].bit_high, 0);
    assert_eq!(regs[0].fields[1].bit_low, 1);
    assert_eq!(regs[0].fields[1].bit_high, 8);

    assert_eq!(regs[1].address_low, 1);
    assert_eq!(regs[1].label, Some("Reg 1".to_string()));
    assert!(regs[1].fields.is_empty());
    Ok(())
}
