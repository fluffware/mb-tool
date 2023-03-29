#[derive(Debug)]
pub enum ByteOrder {
    BigEndian,
    LittleEndian,
}

impl ByteOrder {
    pub fn is_big_endian(&self) -> bool {
        match self {
            Self::BigEndian => true,
            _ => false
        }
    }

    pub fn is_little_endian(&self) -> bool {
        match self {
            Self::LittleEndian => true,
            _ => false
        }
    }
}

#[derive(Debug)]
pub enum WordOrder {
    BigEndian,
    LittleEndian,
}

impl WordOrder {
    pub fn is_big_endian(&self) -> bool {
        match self {
            Self::BigEndian => true,
            _ => false
        }
    }

    pub fn is_little_endian(&self) -> bool {
        match self {
            Self::LittleEndian => true,
            _ => false
        }
    }
}

#[derive(Debug)]
pub enum ValueType {
    Integer { signed: bool },
    Float,
    String { fill: u8 },
}
#[derive(Debug)]
pub struct Encoding {
    pub value: ValueType,
    pub byte_order: ByteOrder,
    pub word_order: WordOrder,
}
