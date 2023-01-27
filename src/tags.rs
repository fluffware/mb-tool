use crate::observable_array::ObservableArray;
use crate::tag_list::TagList;

#[derive(Clone)]
pub struct Tags {
    pub holding_registers: ObservableArray<u16>,
    pub input_registers: ObservableArray<u16>,
    pub discrete_inputs: ObservableArray<bool>,
    pub coils: ObservableArray<bool>,
}

impl Tags {
    pub fn new(init: &TagList) -> Tags {
        let holding_registers = ObservableArray::new(65536);
        let input_registers = ObservableArray::new(65536);
        let discrete_inputs = ObservableArray::new(65536);
        let coils = ObservableArray::new(65536);
        for reg in &init.holding_registers {
            holding_registers.update(reg.address as usize, &[reg.initial_value.unwrap_or(0)]);
        }
        for reg in &init.input_registers {
            input_registers.update(reg.address as usize, &[reg.initial_value.unwrap_or(0)]);
        }
        for bit in &init.coils {
            coils.update(bit.address as usize, &[bit.initial_value.unwrap_or(false)]);
        }
        for bit in &init.discrete_inputs {
            discrete_inputs.update(bit.address as usize, &[bit.initial_value.unwrap_or(false)]);
        }
        Tags {
            holding_registers,
            input_registers,
            discrete_inputs,
            coils,
        }
    }
}
