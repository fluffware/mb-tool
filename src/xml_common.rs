use roxmltree::{Node, TextPos};
use std::error::Error;
use std::str::{FromStr};

pub const NS_V1: &str = "http://www.elektro-kapsel.se/xml/modbus_config/v1";
pub const NS_V2: &str = "http://www.elektro-kapsel.se/xml/modbus_config/v2";

#[derive(Debug)]
pub struct ParseErrorBase<K> where K: std::fmt::Display {
    pub kind: K,
    pub pos: TextPos,
}

impl<K> ParseErrorBase<K> where K: std::fmt::Display {
    pub fn new(node: &Node, kind: K) -> ParseErrorBase<K> {
        ParseErrorBase {
            pos: node.document().text_pos_at(node.range().start),
            kind,
        }
    }
}

impl<K> std::error::Error for ParseErrorBase<K> where K: std::fmt::Display + std::fmt::Debug +  {}

impl<K> std::fmt::Display for ParseErrorBase<K> where K: std::fmt::Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{}:{}: {}", self.pos.row, self.pos.col, self.kind)
    }
}

pub type ParseError = ParseErrorBase<ParseErrorKind>;

use ParseErrorKind::*;

#[derive(Debug)]
pub enum ParseErrorKind {
    WrongNamespace,
    UnexpectedElement,
    UnexpectedText,
    UnexpectedAttribute,
    MissingAttribute(String),
    ParseAttribute(String, Box<dyn Error + Send + Sync>),
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
        }
    }
}

pub fn check_element_ns(node: &Node) -> Result<bool, ParseError> {
    if node.is_element() {
        let ns = node.tag_name().namespace();
        if ns != Some(NS_V1) && ns != Some(NS_V2) {
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

pub fn required_attribute<T>(node: &Node, name: &str) -> Result<T, ParseError>
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

pub fn optional_attribute<T>(node: &Node, name: &str) -> Result<Option<T>, ParseError>
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
