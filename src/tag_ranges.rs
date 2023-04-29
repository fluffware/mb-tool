use crate::range_array::RangeArray;
use crate::tag_list::{RegisterOrGroup, TagList};

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

fn get_ranges(ranges: &mut RangeArray<u16>, base_address: u16, registers: &[RegisterOrGroup]) {
    for reg in registers {
        match reg {
            RegisterOrGroup::Register(r) => {
                ranges.union(&(r.address_low + base_address..r.address_high + base_address + 1));
            }
            RegisterOrGroup::Group(g) => {
                get_ranges(ranges, base_address + g.base_address, &g.registers);
            }
        }
    }
}

impl From<&TagList> for TagRanges {
    fn from(tag_list: &TagList) -> Self {
        let mut ranges = Self::new();
        get_ranges(&mut ranges.input_registers, 0, &tag_list.input_registers);

        get_ranges(
            &mut ranges.holding_registers,
            0,
            &tag_list.holding_registers,
        );

        for bit in &tag_list.discrete_inputs {
            ranges
                .discrete_inputs
                .union(&(bit.address..bit.address + 1));
        }
        for bit in &tag_list.coils {
            ranges.coils.union(&(bit.address..bit.address + 1));
        }
        ranges
    }
}
