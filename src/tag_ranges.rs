use crate::range_array::RangeArray;
use crate::tag_list::TagList;

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

impl Default for TagRanges
{
    fn default() -> Self
    {
	Self::new()
    }
}

impl From<&TagList> for TagRanges
{
    fn from(tag_list: &TagList) -> Self
    {
	let mut ranges = Self::new();
	for reg in &tag_list.input_registers {
	    ranges.input_registers.union(&(reg.address_low..reg.address_high + 1));
	}
	for reg in &tag_list.holding_registers {
	    ranges.holding_registers.union(&(reg.address_low..reg.address_high + 1));
	}
	for bit in &tag_list.discrete_inputs {
	    ranges.discrete_inputs.union(&(bit.address..bit.address + 1));
	}
	for bit in &tag_list.coils {
	    ranges.coils.union(&(bit.address..bit.address + 1));
	}
	ranges
    }
}
