
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
pub enum ValueType {
    Integer{signed: bool},
    Float,
    String{fill: u8},

}
#[derive(Debug)]
pub struct Encoding {
    pub value: ValueType,
    pub byte_order: ByteOrder,
    pub word_order: WordOrder,
}
