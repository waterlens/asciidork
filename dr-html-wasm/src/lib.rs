mod utils;

use asciidork_dr_html_backend as backend;
use asciidork_parser::{parser::ParseResult, prelude::*};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn convert(adoc: &str) -> String {
  let bump = Bump::new();
  let parser = Parser::new(&bump, adoc);
  let result = parser.parse();
  match result {
    Ok(ParseResult { document, .. }) => {
      let html = backend::convert_embedded_article(document).unwrap();
      format!(
        r#"{{"success":true,"html":"{}"}}"#,
        html.replace('"', "\\\"").replace('\n', "\\n")
      )
    }
    Err(diagnostics) => format!(
      r#"{{"success":false,"errors":["{}"]}}"#,
      diagnostics
        .iter()
        .map(Diagnostic::plain_text)
        .collect::<Vec<_>>()
        .join(r#"",""#)
        .replace('"', "\\\"")
        .replace('\n', "\\n")
    ),
  }
}
