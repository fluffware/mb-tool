use super::encoding::Encoding;
use super::presentation::Presentation;

#[derive(Debug)]
pub struct IntegerEnum {
    pub value: u16,
    pub label: String,
}

#[derive(Debug)]
pub struct RegisterRange {
    pub address_low: u16,           // Lowest address for this range
    pub address_high: u16,          // Highesr address for this range, inclusive
    pub label: Option<String>,      // Short description of register
    pub fields: Vec<RegisterField>, // Bit fields if any
    pub initial_value: Option<String>,
    pub presentation: Presentation, // How the value should be displayed
    pub encoding: Encoding,         // How the value is envoded in the range
    pub enums: Vec<IntegerEnum>,    // Enumerated values for this register
}

#[derive(Debug)]
pub struct RegisterField {
    pub bit_low: u8,  // Lowest bit (0 base) in the field
    pub bit_high: u8, // Highest bit in the field, inclusive
    pub label: Option<String>,
    pub presentation: Presentation,
    pub enums: Vec<IntegerEnum>, // Enumerated values for this register
}

pub struct RegisterGroup {
    pub base_address: u16, // Register addresses in this group are offset by this amount
    pub label: Option<String>,
    pub registers: Vec<RegisterOrGroup>,
}

pub enum RegisterOrGroup {
    Register(RegisterRange),
    Group(RegisterGroup),
}

/// Contains inherited values that may affect the register
#[derive(Clone)]
pub struct RegisterContext {
    pub base_address: u16,
}

pub trait RegisterSequence<'a, I>
where
    I: Iterator<Item = (&'a RegisterRange, RegisterContext)>,
{
    fn register_iter(&'a self) -> I;
}

pub struct RegisterIter<'a> {
    pos: Vec<(std::slice::Iter<'a, RegisterOrGroup>, RegisterContext)>,
}

impl<'a> Iterator for RegisterIter<'a> {
    type Item = (&'a RegisterRange, RegisterContext);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((ri, ctxt)) = self.pos.last_mut() {
            match ri.next() {
                Some(RegisterOrGroup::Register(r)) => return Some((r, ctxt.clone())),
                Some(RegisterOrGroup::Group(g)) => {
                    let mut ctxt = ctxt.clone();
                    ctxt.base_address += g.base_address;
                    self.pos.push((g.registers.iter(), ctxt));
                }
                None => {
                    self.pos.pop();
                }
            }
        }
        None
    }
}

impl<'a> RegisterSequence<'a, RegisterIter<'a>> for Vec<RegisterOrGroup> {
    fn register_iter(&'a self) -> RegisterIter<'a> {
        RegisterIter {
            pos: vec![(self.iter(), RegisterContext { base_address: 0 })],
        }
    }
}

pub struct Bit {
    pub address: u16,
    pub label: Option<String>,
    pub initial_value: Option<bool>,
}
pub struct TagList {
    pub input_registers: Vec<RegisterOrGroup>,
    pub holding_registers: Vec<RegisterOrGroup>,
    pub discrete_inputs: Vec<Bit>,
    pub coils: Vec<Bit>,
}
