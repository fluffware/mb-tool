use crate::encoding::{ByteOrder, Encoding, ValueType, WordOrder};
use crate::presentation::Presentation;
use crate::tag_list::{
    Bit, BitOrGroup, Group, IntegerEnum, RegisterField, RegisterOrGroup, RegisterRange, TagOrGroup,
};
use json::{Map, Number, Value};
use serde_json as json;

#[derive(Clone)]
struct BuildContext {
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

fn presentation_attributes(
    map: &mut Map<String, Value>,
    presentation: &Presentation,
)  {
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

fn encoding_attributes(
    map: &mut Map<String, Value>,
    encoding: &Encoding,
) {
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
        "byte-order",
        match encoding.byte_order {
            ByteOrder::LittleEndian => "little",
            ByteOrder::BigEndian => "big",
        },
    );
    map_insert_str(
        map,
        "word-order",
        match encoding.word_order {
            WordOrder::LittleEndian => "little",
            WordOrder::BigEndian => "big",
        },
    );
}

fn addr_to_string(addr: u16, base_addr: u16) -> String {
    if base_addr == 0 {
        format!("{}", addr)
    } else {
        format!("{} ({})", addr + base_addr, addr)
    }
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

/*
fn build_input_field<W: Write>(
    w: &mut W,
    presentation: &Presentation,
    encoding: Option<&Encoding>,
    attrs: &str,
) -> Result {
    let input_type = if let Some(encoding) = encoding {
        match encoding.value {
            ValueType::Integer { .. } => {
                if presentation.radix == 10 {
                    "integer"
                } else {
                    "text"
                }
            }
            ValueType::Float { .. } => "number",
            ValueType::String { .. } => "text",
        }
    } else {
        if presentation.radix == 10 {
            "number"
        } else {
            "text"
        }
    };
    write!(
        w,
        "<input type=\"{input_type}\" class=\"mb_value\" {} {} {}/>\n",
        attrs,
        presantation_attributes(presentation)?,
        if let Some(encoding) = encoding {
            encoding_attributes(encoding)?
        } else {
            "".to_string()
        }
    )?;
    Ok(())
}

fn build_field<W: Write>(
    w: &mut W,
    ctxt: &BuildContext,
    field: &RegisterField,
    register: &RegisterRange,
) -> Result {
    write!(w, r#"<li class="field_item">"#)?;
    if field.bit_low == field.bit_high {
        write!(w, r#"<span class="field_bits">@{}</span>"#, field.bit_low)?;
    } else {
        write!(
            w,
            r#"<span class="field_bits">@{}-{}</span>"#,
            field.bit_high, field.bit_low
        )?;
    }

    if let Some(label) = &field.label {
        write!(w, r#"<span class="field_label">{}</span>"#, esc(&label))?;
    }
    let input_attrs = format!(
        r#"mb:addr-low="{}" mb:addr-high="{}" mb:bit-low="{}" mb:bit-high="{}""#,
        register.address_low + ctxt.base_address,
        register.address_high + ctxt.base_address,
        field.bit_low,
        field.bit_high,
    );
    build_input_field(w, &field.presentation, None, &input_attrs)?;
    if field.bit_low == field.bit_high {
        write!(
            w,
            r#"<input type="checkbox" class="mb_value" mb:addr-low="{}" mb:addr-high="{}" mb:bit-low="{}" mb:bit-high="{}"/>"#,
            register.address_low + ctxt.base_address,
            register.address_high + ctxt.base_address,
            field.bit_low,
            field.bit_high
        )?;
    }
    if !field.enums.is_empty() {
        build_enum_field(w, &field.enums, &input_attrs)?;
    }
    write!(w, "</li>")?;

    Ok(())
}
*/

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
        /*
        let input_attrs = format!(
            r#"mb:addr-low="{}"  mb:addr-high="{}""#,
            self.address_low + ctxt.base_address,
            self.address_high + ctxt.base_address,
        );
        build_input_field(w, &self.presentation, Some(&self.encoding), &input_attrs)?;
        if !self.enums.is_empty() {
            build_enum_field(w, &self.enums, &input_attrs)?;
        }
        if let Some(unit) = &self.presentation.unit {
            write!(w, r#"<span class="unit">{unit}</span>"#)?;
        }
        if !self.fields.is_empty() {
            write!(w, r#"<ul class="field_list">"#)?;
            for f in &self.fields {
                build_field(w, ctxt, f, self)?;
            }
            write!(w, r#"</ul>"#)?;
        }
         */
        Value::Object(map)
    }
}

impl BuildTag for Bit {
    fn build_tag(&self, ctxt: &BuildContext) -> Value {
        let mut map = Map::new();
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
    map_insert(&mut map, "addr", group.base_address+ctxt.base_address);
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

pub fn build_register_list(tags: &Vec<RegisterOrGroup>) -> Value {
    let ctxt = BuildContext { base_address: 0 };
    build_sub_list(&ctxt, tags)
}

pub fn build_bit_list(tags: &Vec<BitOrGroup>) -> Value {
    let ctxt = BuildContext { base_address: 0 };
    build_sub_list(&ctxt, tags)
}

