use crate::observable_array::ObservableArray;
use crate::register_value;
use crate::tag_list::TagList;
use log::error;

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
            if let Some(value_str) = reg.initial_value.as_ref() {
                match register_value::parse(reg, &value_str) {
                    Ok(v) => holding_registers.update(reg.address_low as usize, &v),
                    Err(e) => error!(
                        "Failed to parse initial value for holding register at address {}: {}",
                        reg.address_low, e
                    ),
                }
            }
        }
        for reg in &init.input_registers {
            if let Some(value_str) = reg.initial_value.as_ref() {
                match register_value::parse(reg, &value_str) {
                    Ok(v) => input_registers.update(reg.address_low as usize, &v),
                    Err(e) => error!(
                        "Failed to parse initial value for input register at address {}: {}",
                        reg.address_low, e
                    ),
                }
            }
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
