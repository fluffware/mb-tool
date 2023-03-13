use crate::tag_list::{RegisterField, RegisterRange};
use std::fmt::{Result, Write};

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
    write!(
        w,
        r#"<input type="integer" class="mb_value" mb:addr="{}" mb:bit_low="{}" mb:bit_high="{}"/>"#,
        register.address_low, field.bit_low, field.bit_high
    )?;
    if field.bit_low == field.bit_high {
        write!(
            w,
            r#"<input type="checkbox" class="mb_value" mb:addr="{}" mb:bit_low="{}" mb:bit_high="{}"/>"#,
            register.address_low, field.bit_low, field.bit_high
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
    write!(
        w,
        r#"<input type="integer" class="mb_value" mb:addr-low="{}"  mb:addr-hight="{}" mb:scale="{}"/>"#,
        register.address_low, register.address_high, register.presentation.scale,
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
