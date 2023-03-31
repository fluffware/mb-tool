use crate::encoding::{ByteOrder, Encoding, ValueType, WordOrder};
use crate::presentation::Presentation;
use crate::tag_list::{RegisterField, RegisterRange};
use std::fmt::{Result, Write};

fn presantation_attributes(
    presentation: &Presentation,
) -> std::result::Result<String, std::fmt::Error> {
    let mut output = String::new();
    write!(output, r#" mb:scale="{}""#, presentation.scale)?;
    if let Some(unit) = &presentation.unit {
        write!(output, r#" mb:unit="{}""#, unit)?;
    }
    write!(output, r#" mb:base="{}""#, presentation.base)?;

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

fn build_input_field<W: Write>(
    w: &mut W,
    presentation: &Presentation,
    encoding: Option<&Encoding>,
    attrs: &str,
) -> Result {
    let input_type = if let Some(encoding) = encoding {
        match encoding.value {
            ValueType::Integer { .. } => {
                if presentation.base == 10 {
                    "integer"
                } else {
                    "text"
                }
            }
            ValueType::Float { .. } => "number",
            ValueType::String { .. } => "text",
        }
    } else {
        if presentation.base == 10 {
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

fn build_field<W: Write>(w: &mut W, field: &RegisterField, register: &RegisterRange) -> Result {
    write!(w, r#"<li class="field_item">"#)?;
    if field.bit_low == field.bit_high {
        write!(w, r#"<span class="field_bits">{}</span>"#, field.bit_low)?;
    } else {
        write!(
            w,
            r#"<span class="field_bits">{}-{}</span>"#,
            field.bit_high, field.bit_low
        )?;
    }

    if let Some(label) = &field.label {
        write!(w, r#"<span class="field_label">{label}</span>"#)?;
    }
    let input_attrs = format!(
        r#"mb:addr-low="{}" mb:addr-high="{}" mb:bit-low="{}" mb:bit-high="{}""#,
        register.address_low, register.address_high, field.bit_low, field.bit_high,
    );
    build_input_field(w, &field.presentation, None, &input_attrs)?;
    if field.bit_low == field.bit_high {
        write!(
            w,
            r#"<input type="checkbox" class="mb_value" mb:addr-low="{}" mb:addr-high="{}" mb:bit-low="{}" mb:bit-high="{}"/>"#,
            register.address_low, register.address_high, field.bit_low, field.bit_high
        )?;
    }
    write!(w, "</li>")?;

    Ok(())
}

fn build_register<W: Write>(w: &mut W, register: &RegisterRange) -> Result {
    if register.address_low == register.address_high {
        write!(
            w,
            r#"<span class="register_addr">{}</span>"#,
            register.address_low
        )?;
    } else {
        write!(
            w,
            r#"<span class="register_addr">{} - {}</span>"#,
            register.address_low, register.address_high,
        )?;
    }
    if let Some(label) = &register.label {
        write!(w, r#"<span class="register_label">{label}</span>"#)?;
    }

    let input_attrs = format!(
        r#"mb:addr-low="{}"  mb:addr-high="{}""#,
        register.address_low, register.address_high,
    );
    build_input_field(
        w,
        &register.presentation,
        Some(&register.encoding),
        &input_attrs,
    )?;

    if let Some(unit) = &register.presentation.unit {
        write!(w, r#"<span class="unit">{unit}</span>"#)?;
    }
    if !register.fields.is_empty() {
        write!(w, r#"<ul class="field_list">"#)?;
        for f in &register.fields {
            build_field(w, f, register)?;
        }
        write!(w, r#"</ul>"#)?;
    }
    Ok(())
}

pub fn build_register_list<W: Write>(w: &mut W, registers: &Vec<RegisterRange>) -> Result {
    write!(w, r#"<ul class="register_list">"#)?;
    for r in registers {
        write!(w, r#"<li class="register_item">"#)?;
        build_register(w, r)?;
        write!(w, "</li>")?;
    }
    write!(w, r#"</ul>"#)?;
    Ok(())
}
