#[derive(Debug)]
pub struct Register {
    pub address: u16,
    pub label: Option<String>,
    pub fields: Vec<RegisterField>,
    pub initial_value: Option<u16>,
}

#[derive(Debug)]
pub struct RegisterField {
    pub bit_low: u8,
    pub bit_high: u8,
    pub label: Option<String>,
}

pub struct Bit {
    pub address: u16,
    pub label: Option<String>,
    pub initial_value: Option<bool>,
}
pub struct TagList {
    pub input_registers: Vec<Register>,
    pub holding_registers: Vec<Register>,
    pub discrete_inputs: Vec<Bit>,
    pub coils: Vec<Bit>,
}
