use crate::internal::*;

#[derive(Debug, Default)]
pub struct AsciidoctorHtml {
  html: String,
  alt_html: String,
  footnotes: Vec<(String, String)>,
  doc_attrs: AttrEntries,
  fig_caption_num: usize,
  flags: Flags,
}

impl Backend for AsciidoctorHtml {
  type Output = String;
  type Error = Infallible;

  fn enter_document(&mut self, document: &Document, attrs: &AttrEntries, flags: Flags) {
    self.flags = flags;
    self.doc_attrs = attrs.clone();
    if flags.embedded {
      return;
    }
    self.push_str(r#"<!DOCTYPE html><html"#);
    if !attrs.is_set("nolang") {
      self.push([r#" lang=""#, attrs.str_or("lang", "en"), "\""]);
    }
    let encoding = attrs.str_or("encoding", "UTF-8");
    self.push([r#"><head><meta charset=""#, encoding, r#"">"#]);
    self.push_str(r#"<meta http-equiv="X-UA-Compatible" content="IE=edge">"#);
    self.push_str(r#"<meta name="viewport" content="width=device-width, initial-scale=1.0">"#);
    if !attrs.is_set("reproducible") {
      self.push_str(r#"<meta name="generator" content="Asciidork">"#);
    }
    if let Some(appname) = attrs.str("app-name") {
      self.push([r#"<meta name="application-name" content=""#, appname, "\">"]);
    }
    if let Some(desc) = attrs.str("description") {
      self.push([r#"<meta name="description" content=""#, desc, "\">"]);
    }
    if let Some(keywords) = attrs.str("keywords") {
      self.push([r#"<meta name="keywords" content=""#, keywords, "\">"]);
    }
    if let Some(header) = &document.header {
      if !header.authors.is_empty() {
        self.push_str(r#"<meta name="author" content=""#);
        for (index, author) in header.authors.iter().enumerate() {
          if index > 0 {
            self.push_str(", ");
          }
          // TODO: escape/sanitize, w/ tests, see asciidoctor
          self.push_str(&author.fullname());
        }
        self.push_str(r#"">"#);
      }
    }
    if let Some(_title) = &document.header.as_ref().and_then(|h| h.title.as_ref()) {
      // TODO: strip tags, support doc `title` attr as override
      // @see https://github.com/asciidoctor/asciidoctor/issues/504
      self.push_str(r#"<title>"#);
      self.push_str(r#"</title>"#);
    }

    // self.push_str("<title>Untitled</title>");
    self.push_str(r#"</head><body>"#);
  }

  fn exit_document(&mut self, _document: &Document, _header_attrs: &AttrEntries) {
    if !self.footnotes.is_empty() {
      self.render_footnotes();
    }
    if !self.flags.embedded {
      self.push_str("</body></html>");
    }
  }

  fn enter_paragraph_block(&mut self, block: &Block) {
    self.push_str(r#"<div class="paragraph">"#);
    self.visit_block_title(block.title.as_deref(), None);
  }

  fn exit_paragraph_block(&mut self, _block: &Block) {
    self.push_str("</div>");
  }

  fn enter_simple_block_content(&mut self, _children: &[InlineNode], _block: &Block) {
    self.push_str("<p>");
  }

  fn exit_simple_block_content(&mut self, _children: &[InlineNode], _block: &Block) {
    self.push_str("</p>");
  }

  fn enter_inline_italic(&mut self, _children: &[InlineNode]) {
    self.push_str("<em>");
  }

  fn exit_inline_italic(&mut self, _children: &[InlineNode]) {
    self.push_str("</em>");
  }

  fn visit_inline_text(&mut self, text: &str) {
    self.push_str(text);
  }

  fn visit_joining_newline(&mut self) {
    self.push_ch(' ');
  }

  fn enter_inline_mono(&mut self, _children: &[InlineNode]) {
    self.push_str("<code>");
  }

  fn exit_inline_mono(&mut self, _children: &[InlineNode]) {
    self.push_str("</code>");
  }

  fn enter_inline_bold(&mut self, _children: &[InlineNode]) {
    self.push_str("<strong>");
  }

  fn exit_inline_bold(&mut self, _children: &[InlineNode]) {
    self.push_str("</strong>");
  }

  fn enter_inline_passthrough(&mut self, _children: &[InlineNode]) {}
  fn exit_inline_passthrough(&mut self, _children: &[InlineNode]) {}

  fn visit_inline_specialchar(&mut self, char: &SpecialCharKind) {
    match char {
      SpecialCharKind::Ampersand => self.push_str("&amp;"),
      SpecialCharKind::LessThan => self.push_str("&lt;"),
      SpecialCharKind::GreaterThan => self.push_str("&gt;"),
    }
  }

  fn enter_inline_highlight(&mut self, _children: &[InlineNode]) {
    self.push_str("<mark>");
  }

  fn exit_inline_highlight(&mut self, _children: &[InlineNode]) {
    self.push_str("</mark>");
  }

  fn into_result(self) -> Result<Self::Output, Self::Error> {
    Ok(self.html)
  }

  fn result(&self) -> Result<&Self::Output, Self::Error> {
    Ok(&self.html)
  }

  fn enter_admonition_block(&mut self, kind: AdmonitionKind, block: &Block) {
    let classes = &["admonitionblock", kind.lowercase_str()];
    self.open_element("div", classes, &block.attrs);
    self.push_str(r#"<table><tr><td class="icon"><div class="title">"#);
    self.push_str(kind.str());
    self.push_str(r#"</div></td><td class="content">"#);
    self.visit_block_title(block.title.as_deref(), None);
  }

  fn exit_admonition_block(&mut self, _kind: AdmonitionKind, _block: &Block) {
    self.push_str(r#"</td></tr></table></div>"#);
  }

  fn enter_image_block(&mut self, img_target: &str, img_attrs: &AttrList, block: &Block) {
    let alt = img_attrs.str_positional_at(0).unwrap_or({
      if let Some(captures) = REMOVE_FILE_EXT.captures(img_target) {
        captures.get(1).unwrap().as_str()
      } else {
        img_target
      }
    });
    self.open_element("div", &["imageblock"], &block.attrs);
    self.push_str(r#"<div class="content">"#);
    let mut has_link = false;
    if let Some(href) = &block.attrs.as_ref().and_then(|attrs| attrs.named("link")) {
      self.push([r#"<a class="image" href=""#, *href, r#"">"#]);
      has_link = true;
    }
    self.push([r#"<img src=""#, img_target, r#"" alt=""#, alt, "\""]);
    if let Some(width) = img_attrs.str_positional_at(1) {
      self.push([r#" width=""#, width, "\""]);
    }
    if let Some(height) = img_attrs.str_positional_at(2) {
      self.push([r#" height=""#, height, "\""]);
    }
    self.push_ch('>');
    if has_link {
      self.push_str("</a>");
    }
    self.push_str(r#"</div>"#);
  }

  fn exit_image_block(&mut self, block: &Block) {
    let prefix = if self.doc_attrs.is_unset("figure-caption") {
      None
    } else {
      self.fig_caption_num += 1;
      Some(Cow::Owned(format!("Figure {}. ", self.fig_caption_num)))
    };
    self.visit_block_title(block.title.as_deref(), prefix);
    self.push_str(r#"</div>"#);
  }

  fn visit_document_attribute_decl(&mut self, name: &str, entry: &AttrEntry) {
    self.doc_attrs.insert(name.to_string(), entry.clone());
  }

  fn enter_footnote(&mut self, _id: Option<&str>, _content: &[InlineNode]) {
    mem::swap(&mut self.html, &mut self.alt_html);
  }

  fn exit_footnote(&mut self, id: Option<&str>, _content: &[InlineNode]) {
    mem::swap(&mut self.alt_html, &mut self.html);
    let mut footnote = String::new();
    mem::swap(&mut footnote, &mut self.alt_html);
    let num = (self.footnotes.len() + 1).to_string();
    self.push_str(r#"<sup class="footnote""#);
    if let Some(id) = id {
      self.push([r#" id="_footnote_"#, id, "\""]);
    }
    self.push_str(r#">[<a id="_footnoteref_"#);
    self.push([&num, r##"" class="footnote" href="#_footnotedef_"##, &num]);
    self.push([r#"" title="View footnote.">"#, &num, "</a>]</sup>"]);
    let id = id.unwrap_or(&num);
    self.footnotes.push((id.to_string(), footnote));
  }
}

impl AsciidoctorHtml {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_string(self) -> String {
    self.html
  }

  fn push_str(&mut self, s: &str) {
    self.html.push_str(s);
  }

  fn push_ch(&mut self, c: char) {
    self.html.push(c);
  }

  fn push<const N: usize>(&mut self, strs: [&str; N]) {
    for s in strs {
      self.push_str(s);
    }
  }

  fn visit_block_title(&mut self, title: Option<&str>, prefix: Option<Cow<str>>) {
    if let Some(title) = title {
      self.push_str(r#"<div class="title">"#);
      if let Some(prefix) = prefix {
        self.push_str(prefix.as_ref());
      }
      self.push_str(title);
      self.push_str("</div>");
    }
  }

  fn open_element(&mut self, element: &str, classes: &[&str], attrs: &Option<AttrList>) {
    self.push_ch('<');
    self.push_str(element);
    if let Some(id) = attrs.as_ref().and_then(|a| a.id.as_ref()) {
      self.push_str(" id=\"");
      self.push_str(id);
      self.push_ch('"');
    }
    if !classes.is_empty() || attrs.as_ref().map_or(false, |a| !a.roles.is_empty()) {
      self.push_str(" class=\"");
      for class in classes {
        self.push_str(class);
        self.push_ch(' ');
      }
      if let Some(roles) = attrs.as_ref().map(|a| &a.roles) {
        for role in roles {
          self.push_str(role);
          self.push_ch(' ');
        }
      }
      self.html.pop();
      self.push_ch('"');
    }
    self.push_ch('>');
  }

  fn render_footnotes(&mut self) {
    self.push_str(r#"<div id="footnotes"><hr>"#);
    let footnotes = mem::take(&mut self.footnotes);
    for (index, (_id, footnote)) in footnotes.iter().enumerate() {
      let num = &(index + 1).to_string();
      self.push_str(r#"<div class="footnote" id="_footnotedef_"#);
      self.push([num, r##""><a href="#_footnoteref_"##, num, "\">"]);
      self.push([num, "</a>. ", footnote, "</div>"]);
    }
    self.push_str(r#"</div>"#);
    self.footnotes = footnotes;
  }
}

lazy_static! {
  pub static ref REMOVE_FILE_EXT: Regex = Regex::new(r"^(.*)\.[^.]+$").unwrap();
}
