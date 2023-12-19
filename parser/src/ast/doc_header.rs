use std::collections::HashMap;

use crate::prelude::*;

// https://docs.asciidoctor.org/asciidoc/latest/document/header/
#[derive(Debug, PartialEq, Eq)]
pub struct DocHeader<'bmp> {
  pub title: Option<DocTitle<'bmp>>,
  pub authors: Vec<'bmp, Author<'bmp>>,
  pub revision: Option<Revision<'bmp>>,
  pub attrs: HashMap<String<'bmp>, String<'bmp>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DocTitle<'bmp> {
  pub heading: Vec<'bmp, InlineNode<'bmp>>,
  pub subtitle: Option<Vec<'bmp, InlineNode<'bmp>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Revision<'bmp> {
  pub version: String<'bmp>,
  pub date: Option<String<'bmp>>,
  pub remark: Option<String<'bmp>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Author<'bmp> {
  pub first_name: String<'bmp>,
  pub middle_name: Option<String<'bmp>>,
  pub last_name: String<'bmp>,
  pub email: Option<String<'bmp>>,
}
