use crate::device_list::{DeviceDef, DeviceDefList};
use crate::tag_list_xml::{self, parse_tag_list};
use crate::xml_common::ParseErrorKind::UnexpectedElement;
use crate::xml_common::{self, check_element_ns, required_attribute};
use roxmltree::Node;
use std::num::ParseIntError;
use std::str::FromStr;

pub type ParseError = xml_common::ParseErrorBase<ParseErrorKind>;

impl From<tag_list_xml::ParseError> for ParseError {
    fn from(err: tag_list_xml::ParseError) -> ParseError {
        ParseError {
            pos: err.pos,
            kind: Base(err.kind),
        }
    }
}

impl From<xml_common::ParseError> for ParseError {
    fn from(err: xml_common::ParseError) -> ParseError {
        ParseError {
            pos: err.pos,
            kind: Base(tag_list_xml::ParseErrorKind::Base(err.kind)),
        }
    }
}

#[derive(Debug)]
pub enum ParseErrorKind {
    Base(tag_list_xml::ParseErrorKind),
    DuplicateAddr,
}
use ParseErrorKind::*;

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Base(base) => base.fmt(f),
            DuplicateAddr => write!(
                f,
                "A device with the same address already configured"
            ),
        }
    }
}

struct ParsedU8(u8);
impl FromStr for ParsedU8 {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, ParseIntError> {
        let pos_int = if let Some(s) = s.strip_prefix("0x") {
            u8::from_str_radix(s, 16)?
        } else {
            str::parse(s)?
        };
        Ok(ParsedU8(pos_int))
    }
}

impl From<ParsedU8> for u8 {
    fn from(b: ParsedU8) -> u8 {
        b.0
    }
}

fn parse_device(node: &Node) -> Result<DeviceDef, ParseError> {
    let addr = required_attribute::<ParsedU8>(node, "addr")?.into();
    let tags = parse_tag_list(node)?;

    Ok(DeviceDef { addr, tags })
}

pub fn parse_device_list(node: &Node) -> Result<DeviceDefList, ParseError> {
    let mut devices = DeviceDefList::new();
    for child in node.children() {
        if check_element_ns(&child)? {
            match child.tag_name().name() {
                "device" => {
                    let device = parse_device(&child)?;
                    devices.insert(device);
                }
                _ => {
                    return Err(ParseError::new(
                        &child,
                        Base(tag_list_xml::ParseErrorKind::Base(UnexpectedElement)),
                    ))
                }
            }
        }
    }

    Ok(devices)
}
