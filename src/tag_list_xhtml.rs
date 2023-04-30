use crate::encoding::{ByteOrder, Encoding, ValueType, WordOrder};
use crate::presentation::Presentation;
use crate::tag_list::{IntegerEnum, RegisterField, RegisterGroup, RegisterOrGroup, RegisterRange};
use escaper::encode_minimal as esc;
use std::fmt::{Result, Write};

#[derive(Clone)]
struct BuildContext {
    base_address: u16,
}

fn presantation_attributes(
    presentation: &Presentation,
) -> std::result::Result<String, std::fmt::Error> {
    let mut output = String::new();
    write!(output, r#" mb:scale="{}""#, presentation.scale)?;
    if let Some(unit) = &presentation.unit {
        write!(output, r#" mb:unit="{}""#, unit)?;
    }
    write!(output, r#" mb:radix="{}""#, presentation.radix)?;

    write!(output, r#" mb:decimals="{}""#, presentation.decimals)?;

    Ok(output)
}

fn encoding_attributes(encoding: &Encoding) -> std::result::Result<String, std::fmt::Error> {
    let mut output = String::new();

    match encoding.value {
        ValueType::Integer { signed } => {
            write!(
                output,
                r#" mb:value-type="integer" mb:sign="{}""#,
                if signed { "signed" } else { "unsigned" },
            )?;
        }
        ValueType::Float => {
            write!(output, r#" mb:value-type="float""#)?;
        }
        ValueType::String { fill } => {
            write!(output, r#" mb:value-type="string" mb:fill="{}""#, fill)?;
        }
    }
    write!(
        output,
        r#" mb:byte-order="{}" "#,
        match encoding.byte_order {
            ByteOrder::LittleEndian => "little",
            ByteOrder::BigEndian => "big",
        }
    )?;
    write!(
        output,
        r#" mb:word-order="{}" "#,
        match encoding.word_order {
            WordOrder::LittleEndian => "little",
            WordOrder::BigEndian => "big",
        }
    )?;
    Ok(output)
}

fn addr_to_string(addr: u16, base_addr: u16) -> String {
    if base_addr == 0 {
        format!("{}", addr)
    } else {
        format!("{} ({})", addr + base_addr, addr)
    }
}

fn build_enum_field<W: Write>(w: &mut W, enums: &[IntegerEnum], attrs: &str) -> Result {
    write!(w, "<select class=\"mb_value mb_enum\" {attrs}>\n",)?;
    for e in enums {
        write!(
            w,
            "<option value=\"{}\">{}</option>\n",
            e.value,
            esc(&e.label)
        )?;
    }
    write!(w, "</select>\n",)?;
    Ok(())
}

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

fn build_register<W: Write>(w: &mut W, ctxt: &BuildContext, register: &RegisterRange) -> Result {
    if register.address_low == register.address_high {
        write!(
            w,
            r#"<span class="register_addr">{}</span>"#,
            addr_to_string(register.address_low, ctxt.base_address)
        )?;
    } else {
        write!(
            w,
            r#"<span class="register_addr">{} - {}</span>"#,
            addr_to_string(register.address_low, ctxt.base_address),
            addr_to_string(register.address_high, ctxt.base_address),
        )?;
    }
    if let Some(label) = &register.label {
        write!(w, r#"<span class="register_label">{}</span>"#, esc(&label))?;
    }

    let input_attrs = format!(
        r#"mb:addr-low="{}"  mb:addr-high="{}""#,
        register.address_low + ctxt.base_address,
        register.address_high + ctxt.base_address,
    );
    build_input_field(
        w,
        &register.presentation,
        Some(&register.encoding),
        &input_attrs,
    )?;
    if !register.enums.is_empty() {
        build_enum_field(w, &register.enums, &input_attrs)?;
    }
    if let Some(unit) = &register.presentation.unit {
        write!(w, r#"<span class="unit">{unit}</span>"#)?;
    }
    if !register.fields.is_empty() {
        write!(w, r#"<ul class="field_list">"#)?;
        for f in &register.fields {
            build_field(w, ctxt, f, register)?;
        }
        write!(w, r#"</ul>"#)?;
    }
    Ok(())
}

fn build_group<W: Write>(w: &mut W, ctxt: &BuildContext, group: &RegisterGroup) -> Result {
    write!(
        w,
        r#"<span class="group_addr">{}</span>"#,
        addr_to_string(group.base_address, ctxt.base_address)
    )?;
    if let Some(label) = &group.label {
        write!(w, r#"<span class="group_label">{}</span>"#, esc(&label))?;
    }
    let mut ctxt = ctxt.clone();
    ctxt.base_address += group.base_address;
    build_register_sub_list(w, &ctxt, &group.registers)?;
    Ok(())
}

fn build_register_sub_list<W: Write>(
    w: &mut W,
    ctxt: &BuildContext,
    registers: &Vec<RegisterOrGroup>,
) -> Result {
    write!(w, r#"<ul class="register_list">"#)?;
    for r in registers {
        match r {
            RegisterOrGroup::Register(r) => {
                write!(w, r#"<li class="register_item">"#)?;
                build_register(w, ctxt, r)?;
                write!(w, "</li>")?;
            }
            RegisterOrGroup::Group(g) => {
                write!(w, r#"<li class="register_group">"#)?;
                build_group(w, ctxt, g)?;
                write!(w, "</li>")?;
            }
        }
    }
    write!(w, r#"</ul>"#)?;
    Ok(())
}

pub fn build_register_list<W: Write>(w: &mut W, registers: &Vec<RegisterOrGroup>) -> Result {
    let ctxt = BuildContext { base_address: 0 };
    build_register_sub_list(w, &ctxt, registers)
}
