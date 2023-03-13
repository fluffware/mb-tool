use crate::tag_list::TagList;
use crate::tag_list_xhtml;

use crate::web_server::BuildPage;
use hyper::{Body, Request, Response, StatusCode};
use std::fmt::Write;
use std::sync::{Arc, RwLock};

pub fn build_page(tag_list: Arc<RwLock<TagList>>) -> BuildPage {
    Box::new(move |_req: Request<Body>| {
        let mut w = String::new();
        write!(
            w,
            r#"<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">"#
        )?;
        write!(w, "<xhtml xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:mb=\"http://www.elektro-kapsel.se/xml/mb-tool\">")?;
        write!(w, "<head>")?;
        write!(
            w,
            r#"<link rel="stylesheet" href="/style.css" type="text/css" />
            "#
        )?;
        write!(
            w,
            r#"<script src="/modbus.js"/>
            "#
        )?;
        writeln!(w, "</head>")?;
        writeln!(w, "<body onload=\"setup()\">")?;
        let tag_list = tag_list
            .read()
            .map_err(|_| "Failed to get read lock for tag list")?;

        writeln!(w, "<h2>Holding registers</h2>")?;
        writeln!(w, "<div id=\"holding_registers\">")?;
        tag_list_xhtml::build_register_list(&mut w, &tag_list.holding_registers)?;
        writeln!(w, "\n</div>")?;

        writeln!(w, "<h2>Input registers</h2>")?;
        writeln!(w, "<div id=\"input_registers\">")?;
        tag_list_xhtml::build_register_list(&mut w, &tag_list.input_registers)?;
        writeln!(w, "\n</div>")?;

        writeln!(w, "</body></xhtml>")?;
        
        let resp = Response::builder()
            .header("Content-Type", "application/xhtml+xml")
            .status(StatusCode::OK);
        let resp = resp.body(Body::from(w))?;
        Ok(resp)
    })
}
