use crate::error::DynResult;
use crate::tag_list::TagList;
use crate::web_server::BuildPage;
use crate::tag_list_json;
use handlebars::Handlebars;
use hyper::{Body, Request, Response, StatusCode};
use serde_json::{Map, Value};
use log::{debug, error};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

struct Template {
    engine: Handlebars<'static>,
}

impl Template {
    fn new(template_dir: &Path) -> DynResult<Template> {
        let mut engine = Handlebars::new();
        engine.register_templates_directory(".hbs", template_dir)?;
        Ok(Template { engine })
    }
}

pub fn error_response() -> DynResult<Response<Body>> {
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
    let resp = resp.body(Body::from(w))?;
    Ok(resp)
}

pub fn build_page(tag_list: Arc<RwLock<TagList>>, web_dir: PathBuf) -> DynResult<BuildPage> {
    let templates = Template::new(&web_dir)?;

    Ok(Box::new(move |req: Request<Body>| {
        let template_name = req.uri().path().strip_prefix("/dyn/").unwrap();

        debug!("{template_name}");
        let mut map = Map::new();
        let tags = tag_list.read().unwrap();
        map.insert(
            "holding_registers".to_string(),
            tag_list_json::build_register_list(&tags.holding_registers),
        );
	map.insert(
            "input_registers".to_string(),
            tag_list_json::build_register_list(&tags.input_registers),
        );
	map.insert(
            "coils".to_string(),
            tag_list_json::build_bit_list(&tags.coils),
        );
	map.insert(
            "discrete_inputs".to_string(),
            tag_list_json::build_bit_list(&tags.discrete_inputs),
        );
	
	
        let xml = match templates.engine.render(template_name, &Value::Object(map)) {
            Ok(x) => x,
            Err(e) => {
                error!("Template engine failed: {e}");
                return error_response();
            }
        };
        let resp = Response::builder()
            .header("Content-Type", "application/xhtml+xml")
            .status(StatusCode::OK);
        Ok(resp.body(Body::from(xml))?)
    }))
}
