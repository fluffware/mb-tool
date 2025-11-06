use crate::observable_array::ObservableArray;
use crate::register_value;
use crate::tag_list::TagDefList;
use crate::tag_list::TagSequence;
use crate::range_array::RangeArray;
use log::error;
use std::pin::Pin;

pub enum Updated {
    HoldingRegisters(RangeArray<usize>),
    InputRegisters(RangeArray<usize>),
    DiscreteInputs(RangeArray<usize>),
    Coils(RangeArray<usize>),
}

#[derive(Clone)]
pub struct Tags {
    pub holding_registers: ObservableArray<u16>,
    pub input_registers: ObservableArray<u16>,
    pub discrete_inputs: ObservableArray<bool>,
    pub coils: ObservableArray<bool>,
}

impl Tags {
    pub fn new(init: &TagDefList) -> Tags {
        let holding_registers = ObservableArray::new(65536);
        let input_registers = ObservableArray::new(65536);
        let discrete_inputs = ObservableArray::new(65536);
        let coils = ObservableArray::new(65536);

        for (reg, ctxt) in init.holding_registers.tag_iter() {
            if let Some(value_str) = reg.initial_value.as_ref() {
                match register_value::parse(reg, &value_str) {
                    Ok(v) => {
                        holding_registers.update((reg.address_low + ctxt.base_address) as usize, &v)
                    }
                    Err(e) => error!(
                        "Failed to parse initial value for holding register at address {}: {}",
                        reg.address_low, e
                    ),
                }
            }
        }
        for (reg, ctxt) in init.input_registers.tag_iter() {
            if let Some(value_str) = reg.initial_value.as_ref() {
                match register_value::parse(reg, &value_str) {
                    Ok(v) => {
                        input_registers.update((reg.address_low + ctxt.base_address) as usize, &v)
                    }
                    Err(e) => error!(
                        "Failed to parse initial value for input register at address {}: {}",
                        reg.address_low, e
                    ),
                }
            }
        }

        for (bit, ctxt) in init.coils.tag_iter() {
            coils.update(
                (bit.address + ctxt.base_address) as usize,
                &[bit.initial_value.unwrap_or(false)],
            );
        }
        for (bit, ctxt) in init.discrete_inputs.tag_iter() {
            discrete_inputs.update(
                (bit.address + ctxt.base_address) as usize,
                &[bit.initial_value.unwrap_or(false)],
            );
        }

        Tags {
            holding_registers,
            input_registers,
            discrete_inputs,
            coils,
        }
    }

    pub fn updated(&self) -> Pin<Box<dyn Future<Output = Updated> + Send + 'static>> {
	let holding_registers = self.holding_registers.updated();
	let input_registers =  self.input_registers.updated();
	let discrete_inputs = self.discrete_inputs.updated() ;
	let coils = self.coils.updated();
	Box::pin(async move {
	tokio::select! {
            ranges = holding_registers =>Updated::HoldingRegisters(ranges),
            ranges = input_registers => Updated::InputRegisters(ranges),
            ranges = discrete_inputs=>Updated::DiscreteInputs(ranges),
            ranges = coils => Updated::Coils(ranges),
        }})
    }
}
