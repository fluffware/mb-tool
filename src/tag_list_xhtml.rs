use crate::tag_list::{Register, RegisterField};
use std::fmt::{Result, Write};

fn build_field<W: Write>(w: &mut W, field: &RegisterField, register: &Register) -> Result {
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
        register.address, field.bit_low, field.bit_high
    )?;
    if field.bit_low == field.bit_high {
        write!(
            w,
            r#"<input type="checkbox" class="mb_value" mb:addr="{}" mb:bit_low="{}" mb:bit_high="{}"/>"#,
            register.address, field.bit_low, field.bit_high
        )?;
    }
    write!(w, "</li>")?;

    Ok(())
}

fn build_register<W: Write>(w: &mut W, register: &Register) -> Result {
    write!(
        w,
        r#"<span class="register_addr">{}</span>"#,
        register.address
    )?;
    if let Some(label) = &register.label {
        write!(w, r#"<span class="register_label">{label}</span>"#)?;
    }
    write!(
        w,
        r#"<input type="integer" class="mb_value" mb:addr="{}" mb:scale="{}"/>"#,
        register.address, register.scale,
    )?;
    if let Some(unit) = &register.unit {
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

pub fn build_register_list<W: Write>(w: &mut W, registers: &Vec<Register>) -> Result {
    write!(w, r#"<ul class="register_list">"#)?;
    for r in registers {
        write!(w, r#"<li class="register_item">"#)?;
        build_register(w, r)?;
        write!(w, "</li>")?;
    }
    write!(w, r#"</ul>"#)?;
    Ok(())
}
