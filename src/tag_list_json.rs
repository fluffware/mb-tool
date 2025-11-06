use crate::encoding::{ByteOrder, Encoding, ValueType, WordOrder};
use crate::presentation::Presentation;
use crate::tag_list::{
    Bit, BitOrGroup, Group, IntegerEnum, RegisterField, RegisterOrGroup, RegisterRange, TagOrGroup,
};
use json::{Map, Number, Value};
use serde_json as json;

#[derive(Clone)]
struct BuildContext {
    unit_addr: u8,
    base_address: u16,
}

fn map_insert<V>(map: &mut Map<String, Value>, name: &str, value: V)
where
    Number: From<V>,
{
    map.insert(name.to_string(), Value::Number(Number::from(value)));
}

fn map_insert_str(map: &mut Map<String, Value>, name: &str, value: &str) {
    map.insert(name.to_string(), Value::String(value.to_string()));
}

fn presentation_attributes(map: &mut Map<String, Value>, presentation: &Presentation) {
    map.insert(
        "scale".to_string(),
        Value::Number(Number::from_f64(presentation.scale as f64).unwrap()),
    );
    if let Some(unit) = &presentation.unit {
        map_insert_str(map, "unit", unit);
    }
    map_insert(map, "radix", presentation.radix);
    map_insert(map, "decimals", presentation.decimals);
}

fn encoding_attributes(map: &mut Map<String, Value>, encoding: &Encoding) {
    match encoding.value {
        ValueType::Integer { signed } => {
            map_insert_str(map, "value_type", "integer");
            map_insert_str(map, "sign", if signed { "signed" } else { "unsigned" });
        }
        ValueType::Float => {
            map_insert_str(map, "value_type", "float");
        }
        ValueType::String { fill } => {
            map_insert_str(map, "value_type", "string");
            map_insert(map, "fill", fill);
        }
    }
    map_insert_str(
        map,
        "byte_order",
        match encoding.byte_order {
            ByteOrder::LittleEndian => "little",
            ByteOrder::BigEndian => "big",
        },
    );
    map_insert_str(
        map,
        "word_order",
        match encoding.word_order {
            WordOrder::LittleEndian => "little",
            WordOrder::BigEndian => "big",
        },
    );
}

fn build_enum_field(map: &mut Map<String, Value>, enums: &[IntegerEnum]) {
    let mut json_enums = Vec::new();
    for e in enums {
        let mut enum_map = Map::new();
        map_insert_str(&mut enum_map, "name", &e.label);
        map_insert(&mut enum_map, "value", e.value);
        json_enums.push(Value::Object(enum_map));
    }
    map.insert("enum".to_string(), Value::Array(json_enums));
}

fn build_field(ctxt: &BuildContext, field: &RegisterField, register: &RegisterRange) -> Value {
    let mut map = Map::new();
    if field.bit_low == field.bit_high {
        map_insert(&mut map, "bit", field.bit_low);
    }
    map_insert(&mut map, "bit_low", field.bit_low);
    map_insert(&mut map, "bit_high", field.bit_high);
    if let Some(label) = &field.label {
        map_insert_str(&mut map, "label", label);
    }
    map_insert(&mut map, "unit_addr", ctxt.unit_addr);
    map_insert(
        &mut map,
        "addr_low",
        register.address_low + ctxt.base_address,
    );
    map_insert(
        &mut map,
        "addr_high",
        register.address_high + ctxt.base_address,
    );

    if !field.enums.is_empty() {
        build_enum_field(&mut map, &field.enums);
    }
    Value::Object(map)
}

trait BuildTag {
    fn build_tag(&self, ctxt: &BuildContext) -> Value;
}

impl BuildTag for RegisterRange {
    fn build_tag(&self, ctxt: &BuildContext) -> Value {
        let mut map = Map::new();
        if self.address_low == self.address_high {
            map_insert(&mut map, "addr", self.address_low + ctxt.base_address);
            if ctxt.base_address != 0 {
                map_insert(&mut map, "rel", self.address_low);
            }
        }
        map_insert(&mut map, "unit_addr", ctxt.unit_addr);
        map_insert(&mut map, "addr_low", self.address_low + ctxt.base_address);
        map_insert(&mut map, "addr_high", self.address_high + ctxt.base_address);
        if ctxt.base_address != 0 {
            map_insert(&mut map, "rel_low", self.address_low);
            map_insert(&mut map, "rel_high", self.address_high);
        }

        if let Some(label) = &self.label {
            map_insert_str(&mut map, "label", label);
        }
        presentation_attributes(&mut map, &self.presentation);
        encoding_attributes(&mut map, &self.encoding);

        if !self.enums.is_empty() {
            build_enum_field(&mut map, &self.enums);
        }
        if !self.fields.is_empty() {
            let mut json_fields = Vec::new();
            for f in &self.fields {
                json_fields.push(build_field(ctxt, f, self));
            }
            map.insert("fields".to_string(), Value::Array(json_fields));
        }
        Value::Object(map)
    }
}

impl BuildTag for Bit {
    fn build_tag(&self, ctxt: &BuildContext) -> Value {
        let mut map = Map::new();
	map_insert(&mut map, "unit_addr", ctxt.unit_addr);
        map_insert(&mut map, "addr", self.address + ctxt.base_address);
        if ctxt.base_address != 0 {
            map_insert(&mut map, "rel", self.address);
        }

        if let Some(label) = &self.label {
            map_insert_str(&mut map, "label", label);
        }
        Value::Object(map)
    }
}

fn build_group<T>(ctxt: &BuildContext, group: &Group<T>) -> Value
where
    T: BuildTag,
{
    let mut map = Map::new();
    map_insert(&mut map, "addr", group.base_address + ctxt.base_address);
    if ctxt.base_address != 0 {
        map_insert(&mut map, "rel", group.base_address);
    }
    if let Some(label) = &group.label {
        map_insert_str(&mut map, "label", label);
    }
    let mut ctxt = ctxt.clone();
    ctxt.base_address += group.base_address;
    let children = build_sub_list(&ctxt, &group.tags);
    map.insert("children".to_string(), children);
    Value::Object(map)
}

fn build_sub_list<T>(ctxt: &BuildContext, tags: &Vec<TagOrGroup<T>>) -> Value
where
    T: BuildTag,
{
    let mut items = Vec::new();
    for t in tags {
        let mut item = Map::new();
        match t {
            TagOrGroup::<T>::Tag(t) => {
                item.insert("tag".to_string(), t.build_tag(ctxt));
            }
            TagOrGroup::<T>::Group(g) => {
                item.insert("group".to_string(), build_group(ctxt, g));
            }
        }
        items.push(Value::Object(item));
    }
    Value::Array(items)
}

pub fn build_register_list(unit_addr: u8, tags: &Vec<RegisterOrGroup>) -> Value {
    let ctxt = BuildContext {
        unit_addr,
        base_address: 0,
    };
    build_sub_list(&ctxt, tags)
}

pub fn build_bit_list(unit_addr: u8, tags: &Vec<BitOrGroup>) -> Value {
    let ctxt = BuildContext {
        unit_addr,
        base_address: 0,
    };
    build_sub_list(&ctxt, tags)
}
