use super::encoding::Encoding;
use super::presentation::Presentation;

#[derive(Debug)]
pub struct RegisterRange {
    pub address_low: u16,
    pub address_high: u16,
    pub label: Option<String>,
    pub fields: Vec<RegisterField>,
    pub initial_value: Option<u16>,
    pub presentation: Presentation,
    pub encoding: Encoding,
}

#[derive(Debug)]
pub struct RegisterField {
    pub bit_low: u8,
    pub bit_high: u8,
    pub label: Option<String>,
    pub presentation: Presentation,
}

pub struct Bit {
    pub address: u16,
    pub label: Option<String>,
    pub initial_value: Option<bool>,
}
pub struct TagList {
    pub input_registers: Vec<RegisterRange>,
    pub holding_registers: Vec<RegisterRange>,
    pub discrete_inputs: Vec<Bit>,
    pub coils: Vec<Bit>,
}
