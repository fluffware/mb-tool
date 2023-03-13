
#[derive(Debug)]
pub enum ByteOrder {
    BigEndian,
    LittleEndian,
}

#[derive(Debug)]
pub enum WordOrder {
    BigEndian,
    LittleEndian,
}

#[derive(Debug)]
pub struct Encoding {
    byte_order: ByteOrder,
    word_order: WordOrder,
}
