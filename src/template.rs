use crate::device_list::DeviceDefList;
use crate::error::DynResult;
use crate::tag_list_json;
use crate::web_server::{BuildPage, DynBody, DynResponse};
use handlebars::Handlebars;
use hyper::{body::Incoming, Request, Response, StatusCode};
use log::{debug, error};
use rust_embed::RustEmbed;
use serde_json::{Map, Number, Value};
use std::fmt::Write;
use std::sync::Arc;

struct Template {
    engine: Handlebars<'static>,
}

impl Template {
    fn new<R>() -> DynResult<Template>
    where
        R: RustEmbed,
    {
        let mut engine = Handlebars::new();
        engine.register_embed_templates::<R>()?;
        Ok(Template { engine })
    }
}

pub fn error_response() -> DynResult<DynResponse> {
    let resp = Response::builder()
        .header("Content-Type", "application/xhtml+xml")
        .status(StatusCode::INTERNAL_SERVER_ERROR);
    let mut w = String::new();
    write!(
        w,
        r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">"#
    )?;
    write!(w, "<xhtml xmlns=\"http://www.w3.org/1999/xhtml\">")?;
    write!(w, "<head/>")?;
    write!(w, "<body>")?;
    write!(w, "<h1>Template failure</h1>")?;
    write!(
        w,
        "<p>The server failed to render the page. Check server log for errors.</p>"
    )?;

    writeln!(w, "</body></xhtml>")?;
    let resp = resp.body(Box::new(w) as DynBody)?;
    Ok(resp)
}

pub fn build_page<R>(device_def_list: Arc<DeviceDefList>) -> DynResult<BuildPage>
where
    R: RustEmbed,
{
    let templates = Template::new::<R>()?;

    Ok(Box::new(move |req: Request<Incoming>| {
        let template_name = req.uri().path().strip_prefix("/dyn/").unwrap();

        debug!("{template_name}");
        let mut device_list = Vec::new();
        for device in device_def_list.devices() {
            let mut tag_map = Map::new();
            tag_map.insert(
                "unit_addr".to_string(),
                Value::Number(Number::from(device.addr)),
            );
            let tags = &device.tags;
            tag_map.insert(
                "holding_registers".to_string(),
                tag_list_json::build_register_list(device.addr, &tags.holding_registers),
            );
            tag_map.insert(
                "input_registers".to_string(),
                tag_list_json::build_register_list(device.addr, &tags.input_registers),
            );
            tag_map.insert(
                "coils".to_string(),
                tag_list_json::build_bit_list(device.addr, &tags.coils),
            );
            tag_map.insert(
                "discrete_inputs".to_string(),
                tag_list_json::build_bit_list(device.addr, &tags.discrete_inputs),
            );
            device_list.push(Value::Object(tag_map));
        }
        let xml = match templates
            .engine
            .render(template_name, &Value::Array(device_list))
        {
            Ok(x) => x,
            Err(e) => {
                error!("Template engine failed: {e}");
                return error_response();
            }
        };
        let resp = Response::builder()
            .header("Content-Type", "application/xhtml+xml")
            .status(StatusCode::OK);
        Ok(resp.body(Box::new(xml) as DynBody)?)
    }))
}
