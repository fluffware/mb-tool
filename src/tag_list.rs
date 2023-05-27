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

pub struct Group<T> {
    pub base_address: u16, // Register addresses in this group are offset by this amount
    pub label: Option<String>,
    pub tags: Vec<TagOrGroup<T>>,
}

/// Contains inherited values that may affect the tag
#[derive(Clone)]
pub struct TagContext {
    pub base_address: u16,
}

pub trait TagSequence<'a, I, T>
where
    I: Iterator<Item = (&'a T, TagContext)>,
    T: 'a,
{
    fn tag_iter(&'a self) -> I;
}

pub struct TagIter<'a, T> {
    pos: Vec<(std::slice::Iter<'a, TagOrGroup<T>>, TagContext)>,
}

impl<'a, T> Iterator for TagIter<'a, T> {
    type Item = (&'a T, TagContext);
    fn next(&mut self) -> Option<Self::Item> {
        while let Some((ri, ctxt)) = self.pos.last_mut() {
            match ri.next() {
                Some(TagOrGroup::<T>::Tag(r)) => return Some((r, ctxt.clone())),
                Some(TagOrGroup::<T>::Group(g)) => {
                    let mut ctxt = ctxt.clone();
                    ctxt.base_address += g.base_address;
                    self.pos.push((g.tags.iter(), ctxt));
                }
                None => {
                    self.pos.pop();
                }
            }
        }
        None
    }
}

impl<'a, T> TagSequence<'a, TagIter<'a, T>, T> for Vec<TagOrGroup<T>> {
    fn tag_iter(&'a self) -> TagIter<'a, T> {
        TagIter {
            pos: vec![(self.iter(), TagContext { base_address: 0 })],
        }
    }
}

pub struct Bit {
    pub address: u16,
    pub label: Option<String>,
    pub initial_value: Option<bool>,
}

pub enum TagOrGroup<T> {
    Tag(T),
    Group(Group<T>),
}

pub type RegisterOrGroup = TagOrGroup<RegisterRange>;
pub type BitOrGroup = TagOrGroup<Bit>;

pub struct TagList {
    pub input_registers: Vec<RegisterOrGroup>,
    pub holding_registers: Vec<RegisterOrGroup>,
    pub discrete_inputs: Vec<BitOrGroup>,
    pub coils: Vec<BitOrGroup>,
}
