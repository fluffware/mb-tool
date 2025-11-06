use crate::encoding::{ByteOrder, Encoding, ValueType, WordOrder};
use crate::presentation::Presentation;
use crate::tag_list::{
    Bit, BitOrGroup, Group, IntegerEnum, RegisterField, RegisterOrGroup, RegisterRange, TagDefList,
};
use crate::xml_common::ParseErrorKind::UnexpectedElement;
use crate::xml_common::{self, check_element_ns, optional_attribute, required_attribute};
use roxmltree::Node;

use std::num::ParseIntError;
use std::str::{FromStr, ParseBoolError};

pub type ParseError = xml_common::ParseErrorBase<ParseErrorKind>;

impl From<xml_common::ParseError> for ParseError {
    fn from(err: xml_common::ParseError) -> ParseError {
        ParseError {
            pos: err.pos,
            kind: Base(err.kind),
        }
    }
}
use ParseErrorKind::*;

#[derive(Debug)]
pub enum ParseErrorKind {
    Base(xml_common::ParseErrorKind),
    BitRange,
    InvalidByteOrder,
    InvalidWordOrder,
    InvalidSign,
    InvalidValueType,
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Base(base) => base.fmt(f),
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
                _ => return Err(ParseError::new(&child, Base(UnexpectedElement))),
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
                _ => return Err(ParseError::new(&child, Base(UnexpectedElement))),
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

pub fn parse_reg_group(node: &Node) -> Result<Group<RegisterRange>, ParseError> {
    let base_address = optional_attribute::<ParsedU16>(node, "base-addr")?
        .map(|v| u16::from(v))
        .unwrap_or(0u16);
    let label: Option<String> = optional_attribute(node, "label")?;
    let tags = parse_registers_or_groups(node)?;
    Ok(Group::<RegisterRange> {
        base_address,
        label,
        tags,
    })
}

pub fn parse_registers_or_groups(parent: &Node) -> Result<Vec<RegisterOrGroup>, ParseError> {
    let mut regs = Vec::new();
    for child in parent.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "register" => {
                    let reg = parse_register(&child)?;
                    regs.push(RegisterOrGroup::Tag(reg));
                }
                "register-range" => {
                    let reg = parse_register(&child)?;
                    regs.push(RegisterOrGroup::Tag(reg));
                }
                "group" => {
                    let reg = parse_reg_group(&child)?;
                    regs.push(RegisterOrGroup::Group(reg));
                }

                _ => return Err(ParseError::new(&child, Base(UnexpectedElement))),
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
pub fn parse_bit_group(node: &Node) -> Result<Group<Bit>, ParseError> {
    let base_address = optional_attribute::<ParsedU16>(node, "base-addr")?
        .map(|v| u16::from(v))
        .unwrap_or(0u16);
    let label: Option<String> = optional_attribute(node, "label")?;
    let tags = parse_bits_or_groups(node)?;
    Ok(Group::<Bit> {
        base_address,
        label,
        tags,
    })
}

pub fn parse_bits_or_groups(parent: &Node) -> Result<Vec<BitOrGroup>, ParseError> {
    let mut bits = Vec::new();
    for child in parent.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "bit" => {
                    let bit = parse_bit(&child)?;
                    bits.push(BitOrGroup::Tag(bit));
                }
                "group" => {
                    let g = parse_bit_group(&child)?;
                    bits.push(BitOrGroup::Group(g));
                }

                _ => return Err(ParseError::new(&child, Base(UnexpectedElement))),
            }
        }
    }
    Ok(bits)
}

pub fn parse_tag_list(node: &Node) -> Result<TagDefList, ParseError> {
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
                    let bits = parse_bits_or_groups(&child)?;
                    discrete_inputs = Some(bits);
                }
                "coils" => {
                    let bits = parse_bits_or_groups(&child)?;
                    coils = Some(bits);
                }
                _ => return Err(ParseError::new(&child, Base(UnexpectedElement))),
            }
        }
    }
    Ok(TagDefList {
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
    <holding-registers xmlns="http://www.elektro-kapsel.se/xml/modbus_config/v2">
    <register addr="0" label="Reg 0">
        <field bit="0" label="0.0" />
        <field bit-low="1" bit-high="8" label="0.1-8" />
    </register>
    <register addr="1" label="Reg 1" />
</holding-registers>
"#,
    )
    .unwrap();

    let regs_or_groups = parse_registers_or_groups(&doc.root().first_child().unwrap())?;
    let RegisterOrGroup::Tag(reg) = &regs_or_groups[0] else {
        panic!("Not a tag");
    };
    assert_eq!(reg.address_low, 0);
    assert_eq!(reg.label, Some("Reg 0".to_string()));
    assert_eq!(reg.fields[0].bit_low, 0);
    assert_eq!(reg.fields[0].bit_high, 0);
    assert_eq!(reg.fields[1].bit_low, 1);
    assert_eq!(reg.fields[1].bit_high, 8);

    let RegisterOrGroup::Tag(reg) = &regs_or_groups[1] else {
        panic!("Not a tag");
    };
    assert_eq!(reg.address_low, 1);
    assert_eq!(reg.label, Some("Reg 1".to_string()));
    assert!(reg.fields.is_empty());
    Ok(())
}
