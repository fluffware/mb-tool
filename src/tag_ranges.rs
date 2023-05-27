use crate::range_array::RangeArray;
use crate::tag_list::{BitOrGroup, RegisterOrGroup, TagList};

#[derive(Debug)]
pub struct TagRanges {
    pub holding_registers: RangeArray<u16>,
    pub input_registers: RangeArray<u16>,
    pub discrete_inputs: RangeArray<u16>,
    pub coils: RangeArray<u16>,
}

impl TagRanges {
    pub fn new() -> Self {
        TagRanges {
            holding_registers: RangeArray::new(),
            input_registers: RangeArray::new(),
            discrete_inputs: RangeArray::new(),
            coils: RangeArray::new(),
        }
    }
}

impl Default for TagRanges {
    fn default() -> Self {
        Self::new()
    }
}

fn get_register_ranges(
    ranges: &mut RangeArray<u16>,
    base_address: u16,
    registers: &[RegisterOrGroup],
) {
    for reg in registers {
        match reg {
            RegisterOrGroup::Tag(r) => {
                ranges.union(&(r.address_low + base_address..r.address_high + base_address + 1));
            }
            RegisterOrGroup::Group(g) => {
                get_register_ranges(ranges, base_address + g.base_address, &g.tags);
            }
        }
    }
}

fn get_bit_ranges(ranges: &mut RangeArray<u16>, base_address: u16, bits: &[BitOrGroup]) {
    for reg in bits {
        match reg {
            BitOrGroup::Tag(b) => {
                let addr = b.address + base_address;
                ranges.union(&(addr..addr + 1));
            }
            BitOrGroup::Group(g) => {
                get_bit_ranges(ranges, base_address + g.base_address, &g.tags);
            }
        }
    }
}
impl From<&TagList> for TagRanges {
    fn from(tag_list: &TagList) -> Self {
        let mut ranges = Self::new();
        get_register_ranges(&mut ranges.input_registers, 0, &tag_list.input_registers);

        get_register_ranges(
            &mut ranges.holding_registers,
            0,
            &tag_list.holding_registers,
        );

        get_bit_ranges(&mut ranges.discrete_inputs, 0, &tag_list.discrete_inputs);
        get_bit_ranges(&mut ranges.coils, 0, &tag_list.coils);

        ranges
    }
}
