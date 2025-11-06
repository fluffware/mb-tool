use crate::encoding::{Encoding, ValueType};
use crate::tag_list::RegisterRange;
use num_bigint::{BigUint, ParseBigIntError};
use num_traits::cast::FromPrimitive;
use num_traits::Num;
use std::num::ParseFloatError;

#[derive(Debug)]
pub enum ParseError {
    InvalidFloatValue,
    InvalidIntegerValue,
    TooBig,
    Negative,
    InvalidFloatLength,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            Self::InvalidFloatValue => write!(f, "Invalid floating point value"),
            Self::InvalidIntegerValue => write!(f, "Invalid integer value"),
            Self::TooBig => write!(f, "Value does not fit in register range"),
            Self::Negative => write!(f, "Negative values not allowed"),
            Self::InvalidFloatLength => {
                write!(f, "Floating point values must be 2 or 4 registers long.")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl From<ParseFloatError> for ParseError {
    fn from(_e: ParseFloatError) -> Self {
        Self::InvalidFloatValue
    }
}

impl From<ParseBigIntError> for ParseError {
    fn from(_e: ParseBigIntError) -> Self {
        Self::InvalidIntegerValue
    }
}

fn build_words_from_le(bytes: &[u8], encoding: &Encoding) -> Vec<u16> {
    // Length must be even
    assert!(bytes.len() & 1 == 0);
    let mut words = Vec::new();
    let mut high_first = encoding.byte_order.is_big_endian();
    let mut bi: Box<dyn Iterator<Item = u8>> = if encoding.word_order.is_big_endian() {
        Box::new(bytes.iter().cloned().rev())
    } else {
        high_first = !high_first;
        Box::new(bytes.iter().cloned())
    };

    while let Some(first) = bi.next() {
        let second = bi.next().unwrap();
        let w = if high_first {
            u16::from(second) | u16::from(first) << 8
        } else {
            u16::from(first) | u16::from(second) << 8
        };

        words.push(w);
    }
    words
}

pub fn parse(regs: &RegisterRange, value_str: &str) -> Result<Vec<u16>, ParseError> {
    let reg_value: BigUint;
    let word_count = regs.address_high - regs.address_low + 1;
    let words;
    match regs.encoding.value {
        ValueType::Integer { signed } => {
            let neg;
            if regs.presentation.scale != 1.0 {
                let v: f64 = str::parse(value_str)?;
                let scaled = v * f64::from(regs.presentation.scale);
                neg = scaled < 0.0;
                reg_value =
                    BigUint::from_f64(scaled.abs().round()).ok_or(ParseError::InvalidFloatValue)?;
            } else {
                let pos_str;
                (neg, pos_str) = if let Some(s) = value_str.strip_prefix('-') {
                    (true, s)
                } else {
                    (false, value_str)
                };

                let (base, pos_str) = if let Some(s) = pos_str.strip_prefix("0x") {
                    (16, s)
                } else if let Some(s) = pos_str.strip_prefix("0b") {
                    (2, s)
                } else {
                    (10, pos_str)
                };
                reg_value = BigUint::from_str_radix(pos_str, base)?;
            }

            if neg && !signed {
                return Err(ParseError::Negative);
            }
            let mut bytes = reg_value.to_bytes_le();
            if bytes.len() > usize::from(word_count * 2) {
                return Err(ParseError::TooBig);
            }
            // Pad with 0u8
            while bytes.len() < usize::from(word_count) * 2 {
                bytes.push(0);
            }

            if neg {
                let mut carry = 1;
                for b in bytes.iter_mut() {
                    if *b == 0 && carry == 1 {
                        *b = 0;
                        carry = 1;
                    } else {
                        *b = !*b + carry;
                        carry = 0;
                    }
                }
            }
            words = build_words_from_le(&bytes, &regs.encoding)
        }
        ValueType::Float => match word_count {
            2 => {
                let v: f32 = str::parse(value_str)?;

                words = build_words_from_le(&v.to_le_bytes(), &regs.encoding);
            }
            4 => {
                let v: f64 = str::parse(value_str)?;
                words = build_words_from_le(&v.to_le_bytes(), &regs.encoding);
            }
            _ => return Err(ParseError::InvalidFloatLength),
        },
        ValueType::String { fill } => {
            let high_first = regs.encoding.byte_order.is_big_endian();
            words = {
                let mut words = Vec::new();
                let mut bytes = value_str
                    .bytes()
                    .chain(std::iter::repeat(fill))
                    .take(usize::from(word_count) * 2);
                while let Some(first) = bytes.next() {
                    let second = bytes.next().unwrap();
                    let w = if high_first {
                        u16::from(second) | u16::from(first) << 8
                    } else {
                        u16::from(first) | u16::from(second) << 8
                    };

                    words.push(w);
                }
                words
            }
        }
    }
    Ok(words)
}

#[cfg(test)]
mod test {

    use super::parse;
    use crate::encoding::ByteOrder;
    use crate::encoding::Encoding;
    use crate::encoding::ValueType;
    use crate::encoding::WordOrder;
    use crate::presentation::Presentation;
    use crate::tag_list::RegisterRange;

    #[test]
    fn test_parse() {
        let mut reg = RegisterRange {
            address_low: 2,
            address_high: 3,
            label: None,
            fields: Vec::new(),
            initial_value: None,
            presentation: Presentation {
                radix: 10,
                decimals: 0,
                scale: 1.0,
                unit: None,
            },
            encoding: Encoding {
                value: ValueType::Integer { signed: false },
                byte_order: ByteOrder::BigEndian,
                word_order: WordOrder::BigEndian,
            },
            enums: Vec::new(),
        };
        assert_eq!(&parse(&reg, "8933224").unwrap(), &[0x0088, 0x4f68]);
        reg.encoding.byte_order = ByteOrder::LittleEndian;
        assert_eq!(&parse(&reg, "8933224").unwrap(), &[0x8800, 0x684f]);
        reg.encoding.word_order = WordOrder::LittleEndian;
        assert_eq!(&parse(&reg, "8933224").unwrap(), &[0x684f, 0x8800]);
        reg.encoding.byte_order = ByteOrder::BigEndian;
        assert_eq!(&parse(&reg, "8933224").unwrap(), &[0x4f68, 0x0088]);
        reg.encoding.word_order = WordOrder::BigEndian;
        reg.presentation.scale = 10.0;
        assert_eq!(&parse(&reg, "89.3").unwrap(), &[0, 893]);
        reg.presentation.scale = 1.0;
        reg.address_high = 6;
        assert_eq!(
            &parse(&reg, "0x99829387177729893892").unwrap(),
            &[0x9982, 0x9387, 0x1777, 0x2989, 0x3892]
        );
        reg.encoding.value = ValueType::Integer { signed: true };

        assert_eq!(
            &parse(&reg, "-0x39829387177729893892").unwrap(),
            &[!0x3982, !0x9387, !0x1777, !0x2989, !0x3892 + 1]
        );
        assert_eq!(&parse(&reg, "-0b0").unwrap(), &[0, 0, 0, 0, 0]);
        reg.address_high = 2;
        assert_eq!(
            &parse(&reg, "0b1001010011100110").unwrap(),
            &[0b1001010011100110]
        );
        assert_eq!(
            &parse(&reg, "-0b0001010011100110").unwrap(),
            &[-0b0001010011100110i16 as u16]
        );
    }
}
