use std::collections::HashSet;
use std::fmt::Write;

use crate::internal::*;
use crate::str_util;
use EphemeralState::*;

#[derive(Debug, Default)]
pub struct AsciidoctorHtml {
  pub(crate) html: String,
  pub(crate) alt_html: String,
  pub(crate) footnotes: Vec<(u16, String, String)>,
  pub(crate) doc_meta: DocumentMeta,
  pub(crate) fig_caption_num: usize,
  pub(crate) table_caption_num: usize,
  pub(crate) list_stack: Vec<bool>,
  pub(crate) default_newlines: Newlines,
  pub(crate) newlines: Newlines,
  pub(crate) state: HashSet<EphemeralState>,
  pub(crate) autogen_conum: u8,
  pub(crate) in_asciidoc_table_cell: bool,
  pub(crate) section_nums: [u16; 5],
  pub(crate) section_num_levels: isize,
}

impl Backend for AsciidoctorHtml {
  type Output = String;
  type Error = Infallible;

  fn enter_document(&mut self, document: &Document) {
    self.doc_meta = document.meta.clone();
    self.section_num_levels = document.meta.isize("sectnumlevels").unwrap_or(3);
    if document.meta.is_true("hardbreaks-option") {
      self.default_newlines = Newlines::JoinWithBreak
    }

    if !self.standalone() {
      return;
    }
    self.push_str(r#"<!DOCTYPE html><html"#);
    if !document.meta.is_true("nolang") {
      self.push([r#" lang=""#, document.meta.str_or("lang", "en"), "\""]);
    }
    let encoding = document.meta.str_or("encoding", "UTF-8");
    self.push([r#"><head><meta charset=""#, encoding, r#"">"#]);
    self.push_str(r#"<meta http-equiv="X-UA-Compatible" content="IE=edge">"#);
    self.push_str(r#"<meta name="viewport" content="width=device-width, initial-scale=1.0">"#);
    if !document.meta.is_true("reproducible") {
      self.push_str(r#"<meta name="generator" content="Asciidork">"#);
    }
    if let Some(appname) = document.meta.str("app-name") {
      self.push([r#"<meta name="application-name" content=""#, appname, "\">"]);
    }
    if let Some(desc) = document.meta.str("description") {
      self.push([r#"<meta name="description" content=""#, desc, "\">"]);
    }
    if let Some(keywords) = document.meta.str("keywords") {
      self.push([r#"<meta name="keywords" content=""#, keywords, "\">"]);
    }
    if let Some(copyright) = document.meta.str("copyright") {
      self.push([r#"<meta name="copyright" content=""#, copyright, "\">"]);
    }
    self.render_favicon(&document.meta);
    self.render_authors(document.meta.authors());
    self.render_title(document, &document.meta);
    // TODO: stylesheets
    self.push([
      r#"</head><body class=""#,
      document.meta.get_doctype().to_str(),
    ]);
    match document.toc.as_ref().map(|toc| &toc.position) {
      Some(TocPosition::Left) => self.push_str(" toc2 toc-left"),
      Some(TocPosition::Right) => self.push_str(" toc2 toc-right"),
      _ => {}
    }
    self.push_str("\">");
  }

  fn exit_document(&mut self, _document: &Document) {
    if !self.footnotes.is_empty() {
      self.render_footnotes();
    }
    if self.standalone() {
      self.push_str("</body></html>");
    }
  }

  fn enter_header(&mut self) {
    if !self.doc_meta.embedded && !self.doc_meta.is_true("noheader") {
      self.push_str(r#"<div id="header">"#)
    }
  }

  fn exit_header(&mut self) {
    if !self.doc_meta.embedded && !self.doc_meta.is_true("noheader") {
      self.push_str("</div>")
    }
  }

  fn enter_content(&mut self) {
    if !self.doc_meta.embedded {
      self.push_str(r#"<div id="content">"#)
    }
  }

  fn exit_content(&mut self) {
    if !self.doc_meta.embedded {
      self.push_str("</div>")
    }
  }

  fn enter_footer(&mut self) {
    if !self.doc_meta.embedded && !self.doc_meta.is_true("nofooter") {
      self.push_str(r#"<div id="footer">"#)
    }
  }

  fn exit_footer(&mut self) {
    if !self.doc_meta.embedded && !self.doc_meta.is_true("nofooter") {
      self.push_str("</div>")
    }
  }

  fn enter_document_title(&mut self, _nodes: &[InlineNode]) {
    if self.render_doc_title() {
      self.push_str("<h1>")
    } else {
      self.start_buffering();
    }
  }

  fn exit_document_title(&mut self, _nodes: &[InlineNode]) {
    if self.render_doc_title() {
      self.push_str("</h1>");
    } else {
      self.take_buffer(); // discard
    }
    self.render_document_authors();
  }

  fn enter_toc(&mut self, toc: &TableOfContents) {
    self.push_str(r#"<div id="toc" class="toc"#);
    if matches!(toc.position, TocPosition::Left | TocPosition::Right) {
      self.push_ch('2'); // `toc2` roughly means "toc-aside", per dr src
    }
    self.push_str(r#""><div id="toctitle">"#);
    self.push_str(&toc.title);
    self.push_str("</div>");
  }

  fn exit_toc(&mut self, _toc: &TableOfContents) {
    self.push_str("</div>");
  }

  fn enter_toc_level(&mut self, level: u8, _nodes: &[TocNode]) {
    self.push(["<ul class=\"sectlevel", &num_str!(level), "\">"]);
  }

  fn exit_toc_level(&mut self, _level: u8, _nodes: &[TocNode]) {
    self.push_str("</ul>");
  }

  fn enter_toc_node(&mut self, node: &TocNode) {
    self.push_str("<li><a href=\"#");
    if let Some(id) = &node.id {
      self.push_str(id);
    }
    self.push_str("\">")
  }

  fn exit_toc_node(&mut self, _node: &TocNode) {
    self.push_str("</li>");
  }

  fn exit_toc_content(&mut self, _content: &[InlineNode]) {
    self.push_str("</a>");
  }

  fn enter_preamble(&mut self, _blocks: &[Block]) {
    self.push_str(r#"<div id="preamble"><div class="sectionbody">"#);
  }

  fn exit_preamble(&mut self, _blocks: &[Block]) {
    self.push_str("</div></div>");
  }

  fn enter_section(&mut self, section: &Section) {
    let mut section_tag = OpenTag::without_id("div", section.meta.attrs.as_ref());
    section_tag.push_class(section::class(section));
    self.push_open_tag(section_tag);
  }

  fn exit_section(&mut self, section: &Section) {
    if section.level == 1 {
      self.push_str("</div>");
    }
    self.push_str("</div>");
  }

  fn enter_section_heading(&mut self, section: &Section) {
    let level_str = num_str!(section.level + 1);
    if let Some(id) = &section.id {
      self.push(["<h", &level_str, r#" id=""#, id, "\">"]);
    } else {
      self.push(["<h", &level_str, ">"]);
    }
    if self.should_number_section(section) {
      let prefix = section::number_prefix(section.level, &mut self.section_nums);
      self.push_str(&prefix);
    }
  }

  fn exit_section_heading(&mut self, section: &Section) {
    let level_str = num_str!(section.level + 1);
    self.push(["</h", &level_str, ">"]);
    if section.level == 1 {
      self.push_str(r#"<div class="sectionbody">"#);
    }
  }

  fn enter_block_title(&mut self, _title: &[InlineNode], _block: &Block) {
    self.start_buffering();
  }

  fn exit_block_title(&mut self, _title: &[InlineNode], _block: &Block) {
    self.stop_buffering();
  }

  fn enter_compound_block_content(&mut self, _children: &[Block], _block: &Block) {}
  fn exit_compound_block_content(&mut self, _children: &[Block], _block: &Block) {}

  fn enter_simple_block_content(&mut self, _children: &[InlineNode], block: &Block) {
    if block.context == BlockContext::Verse {
      self.newlines = Newlines::Preserve;
    } else if block.has_attr_option("hardbreaks") {
      self.newlines = Newlines::JoinWithBreak;
    }
  }

  fn exit_simple_block_content(&mut self, _children: &[InlineNode], _block: &Block) {
    self.newlines = self.default_newlines;
  }

  fn enter_sidebar_block(&mut self, block: &Block, _content: &BlockContent) {
    self.open_element("div", &["sidebarblock"], block.meta.attrs.as_ref());
    self.push_str(r#"<div class="content">"#);
  }

  fn exit_sidebar_block(&mut self, _block: &Block, _content: &BlockContent) {
    self.push_str("</div></div>");
  }

  fn enter_listing_block(&mut self, block: &Block, _content: &BlockContent) {
    self.open_element("div", &["listingblock"], block.meta.attrs.as_ref());
    self.push_str(r#"<div class="content"><pre"#);
    if let Some(lang) = self.source_lang(block) {
      self.push([
        r#" class="highlight"><code class="language-"#,
        &lang,
        r#"" data-lang=""#,
        &lang,
        r#"">"#,
      ]);
      self.state.insert(IsSourceBlock);
    } else {
      self.push_ch('>');
    }
    self.newlines = Newlines::Preserve;
  }

  fn exit_listing_block(&mut self, _block: &Block, _content: &BlockContent) {
    if self.state.remove(&IsSourceBlock) {
      self.push_str("</code>");
    }
    self.push_str("</pre></div></div>");
    self.newlines = self.default_newlines;
  }

  fn enter_literal_block(&mut self, block: &Block, _content: &BlockContent) {
    self.open_element("div", &["literalblock"], block.meta.attrs.as_ref());
    self.push_str(r#"<div class="content"><pre>"#);
    self.newlines = Newlines::Preserve;
  }

  fn exit_literal_block(&mut self, _block: &Block, _content: &BlockContent) {
    self.push_str("</pre></div></div>");
    self.newlines = self.default_newlines;
  }

  fn enter_passthrough_block(&mut self, _block: &Block, _content: &BlockContent) {}
  fn exit_passthrough_block(&mut self, _block: &Block, _content: &BlockContent) {}

  fn enter_quoted_paragraph(&mut self, block: &Block, _attr: &str, _cite: Option<&str>) {
    self.open_element("div", &["quoteblock"], block.meta.attrs.as_ref());
    self.render_block_title(&block.meta);
    self.push_str("<blockquote>");
  }

  fn exit_quoted_paragraph(&mut self, _block: &Block, attr: &str, cite: Option<&str>) {
    self.exit_attributed(BlockContext::BlockQuote, Some(attr), cite);
  }

  fn enter_quote_block(&mut self, block: &Block, _content: &BlockContent) {
    self.open_element("div", &["quoteblock"], block.meta.attrs.as_ref());
    self.render_block_title(&block.meta);
    self.push_str("<blockquote>");
  }

  fn exit_quote_block(&mut self, block: &Block, _content: &BlockContent) {
    if let Some(attrs) = &block.meta.attrs {
      self.exit_attributed(
        block.context,
        attrs.str_positional_at(1),
        attrs.str_positional_at(2),
      );
    } else {
      self.exit_attributed(block.context, None, None);
    }
  }

  fn enter_verse_block(&mut self, block: &Block, _content: &BlockContent) {
    self.open_element("div", &["verseblock"], block.meta.attrs.as_ref());
    self.render_block_title(&block.meta);
    self.push_str(r#"<pre class="content">"#);
  }

  fn exit_verse_block(&mut self, block: &Block, content: &BlockContent) {
    self.exit_quote_block(block, content)
  }

  fn enter_example_block(&mut self, block: &Block, _content: &BlockContent) {
    if block.has_attr_option("collapsible") {
      self.open_element("details", &[], block.meta.attrs.as_ref());
      if block.has_attr_option("open") {
        self.html.pop();
        self.push_str(" open>");
      }
      self.push_str(r#"<summary class="title">"#);
      if block.meta.title.is_some() {
        self.push_buffered();
      } else {
        self.push_str("Details");
      }
      self.push_str("</summary>");
    } else {
      self.open_element("div", &["exampleblock"], block.meta.attrs.as_ref());
    }
    self.push_str(r#"<div class="content">"#);
  }

  fn exit_example_block(&mut self, block: &Block, _content: &BlockContent) {
    if block.has_attr_option("collapsible") {
      self.push_str("</div></details>");
    } else {
      self.push_str("</div></div>");
    }
  }

  fn enter_open_block(&mut self, block: &Block, _content: &BlockContent) {
    self.open_element("div", &["openblock"], block.meta.attrs.as_ref());
    self.push_str(r#"<div class="content">"#);
  }

  fn exit_open_block(&mut self, _block: &Block, _content: &BlockContent) {
    self.push_str("</div></div>");
  }

  fn enter_discrete_heading(&mut self, level: u8, id: Option<&str>, block: &Block) {
    let level_str = num_str!(level + 1);
    if let Some(id) = id {
      self.push(["<h", &level_str, r#" id=""#, id, "\""]);
    } else {
      self.push(["<h", &level_str]);
    }
    self.push_str(r#" class="discrete"#);
    if let Some(roles) = block.meta.attrs.as_ref().map(|a| &a.roles) {
      for role in roles {
        self.push_ch(' ');
        self.push_str(role);
      }
    }
    self.push_str("\">");
  }

  fn exit_discrete_heading(&mut self, level: u8, _id: Option<&str>, _block: &Block) {
    self.push(["</h", &num_str!(level + 1), ">"]);
  }

  fn enter_unordered_list(&mut self, block: &Block, items: &[ListItem], _depth: u8) {
    let attrs = block.meta.attrs.as_ref();
    let custom = attrs.and_then(|a| a.unordered_list_custom_marker_style());
    let interactive = attrs.map(|a| a.has_option("interactive")).unwrap_or(false);
    self.list_stack.push(interactive);
    let mut div = OpenTag::new("div", attrs);
    let mut ul = OpenTag::new("ul", None);
    div.push_class("ulist");
    if let Some(custom) = custom {
      div.push_class(custom);
      ul.push_class(custom);
    }
    if items.iter().any(ListItem::is_checklist) {
      div.push_class("checklist");
      ul.push_class("checklist");
    }
    self.push_open_tag(div);
    self.render_block_title(&block.meta);
    self.push_open_tag(ul);
  }

  fn exit_unordered_list(&mut self, _block: &Block, _items: &[ListItem], _depth: u8) {
    self.list_stack.pop();
    self.push_str("</ul></div>");
  }

  fn enter_callout_list(&mut self, block: &Block, _items: &[ListItem], _depth: u8) {
    self.autogen_conum = 1;
    self.open_element("div", &["colist arabic"], block.meta.attrs.as_ref());
    self.push_str(if self.doc_meta.icon_mode() != IconMode::Text { "<table>" } else { "<ol>" });
  }

  fn exit_callout_list(&mut self, _block: &Block, _items: &[ListItem], _depth: u8) {
    self.push_str(if self.doc_meta.icon_mode() != IconMode::Text {
      "</table></div>"
    } else {
      "</ol></div>"
    });
  }

  fn enter_description_list(&mut self, block: &Block, _items: &[ListItem], _depth: u8) {
    self.open_element("div", &["dlist"], block.meta.attrs.as_ref());
    self.push_str("<dl>");
  }

  fn exit_description_list(&mut self, _block: &Block, _items: &[ListItem], _depth: u8) {
    self.push_str("</dl></div>");
  }

  fn enter_description_list_term(&mut self, _item: &ListItem) {
    self.push_str(r#"<dt class="hdlist1">"#);
  }

  fn exit_description_list_term(&mut self, _item: &ListItem) {
    self.push_str("</dt>");
  }

  fn enter_description_list_description(&mut self, blocks: &[Block], _item: &ListItem) {
    if blocks.first().map_or(false, |block| {
      block.context == BlockContext::Paragraph && matches!(block.content, BlockContent::Simple(_))
    }) {
      self.state.insert(VisitingSimpleTermDescription);
    }
    self.push_str("<dd>");
  }

  fn exit_description_list_description(&mut self, _blocks: &[Block], _item: &ListItem) {
    self.push_str("</dd>");
  }

  fn enter_ordered_list(&mut self, block: &Block, items: &[ListItem], depth: u8) {
    self.list_stack.push(false);
    let attrs = block.meta.attrs.as_ref();
    let custom = attrs.and_then(|attrs| attrs.ordered_list_custom_number_style());
    let list_type = custom
      .and_then(list_type_from_class)
      .unwrap_or_else(|| list_type_from_depth(depth));
    let class = custom.unwrap_or_else(|| list_class_from_depth(depth));
    let classes = &["olist", class];
    self.open_element("div", classes, block.meta.attrs.as_ref());
    self.render_block_title(&block.meta);
    self.push([r#"<ol class=""#, class, "\""]);

    if list_type != "1" {
      self.push([" type=\"", list_type, "\""]);
    }

    if let Some(attr_start) = attrs.and_then(|attrs| attrs.named("start")) {
      self.push([" start=\"", attr_start, "\""]);
    } else {
      match items[0].marker {
        ListMarker::Digits(1) => {}
        ListMarker::Digits(n) => {
          // TODO: asciidoctor documents that this is OK,
          // but it doesn't actually work, and emits a warning
          self.push([" start=\"", &num_str!(n), "\""]);
        }
        _ => {}
      }
    }

    if block.has_attr_option("reversed") {
      self.push_str(" reversed>");
    } else {
      self.push_str(">");
    }
  }

  fn exit_ordered_list(&mut self, _block: &Block, _items: &[ListItem], _depth: u8) {
    self.list_stack.pop();
    self.push_str("</ol></div>");
  }

  fn enter_list_item_principal(&mut self, item: &ListItem, list_variant: ListVariant) {
    if list_variant != ListVariant::Callout || self.doc_meta.icon_mode() == IconMode::Text {
      self.push_str("<li><p>");
      self.render_checklist_item(item);
    } else {
      self.push_str("<tr><td>");
      let n = item.marker.callout_num().unwrap_or(self.autogen_conum);
      self.autogen_conum = n + 1;
      if self.doc_meta.icon_mode() == IconMode::Font {
        self.push_callout_number_font(n);
      } else {
        self.push_callout_number_img(n);
      }
      self.push_str("</td><td>");
    }
  }

  fn exit_list_item_principal(&mut self, _item: &ListItem, list_variant: ListVariant) {
    if list_variant != ListVariant::Callout || self.doc_meta.icon_mode() == IconMode::Text {
      self.push_str("</p>");
    } else {
      self.push_str("</td>");
    }
  }

  fn enter_list_item_blocks(&mut self, _: &[Block], _: &ListItem, _: ListVariant) {}

  fn exit_list_item_blocks(&mut self, _blocks: &[Block], _items: &ListItem, variant: ListVariant) {
    if variant != ListVariant::Callout || self.doc_meta.icon_mode() == IconMode::Text {
      self.push_str("</li>");
    } else {
      self.push_str("</tr>");
    }
  }

  fn enter_paragraph_block(&mut self, block: &Block) {
    if self.doc_meta.get_doctype() != DocType::Inline {
      if !self.state.contains(&VisitingSimpleTermDescription) {
        self.open_element("div", &["paragraph"], block.meta.attrs.as_ref());
        self.render_block_title(&block.meta);
      }
      self.push_str("<p>");
    }
  }

  fn exit_paragraph_block(&mut self, _block: &Block) {
    if self.doc_meta.get_doctype() != DocType::Inline {
      self.push_str("</p>");
      if !self.state.contains(&VisitingSimpleTermDescription) {
        self.push_str("</div>");
      }
      self.state.remove(&VisitingSimpleTermDescription);
    }
  }

  fn enter_table(&mut self, table: &Table, block: &Block) {
    self.open_table_element(block);
    self.table_caption(block);
    self.push_str("<colgroup>");
    let autowidth = block.meta.has_attr_option("autowidth");
    for width in table.col_widths.distribute() {
      self.push_str("<col");
      if !autowidth {
        if let DistributedColWidth::Percentage(width) = width {
          if width.fract() == 0.0 {
            write!(self.html, r#" style="width: {}%;""#, width).unwrap();
          } else {
            let width_s = format!("{:.4}", width);
            let width_s = width_s.trim_end_matches('0');
            write!(self.html, r#" style="width: {width_s}%;""#).unwrap();
          }
        }
      }
      self.push_ch('>');
    }
    self.push_str("</colgroup>");
  }

  fn exit_table(&mut self, _table: &Table, _block: &Block) {
    self.push_str("</table>");
  }

  fn asciidoc_table_cell_backend(&mut self) -> Self {
    Self {
      in_asciidoc_table_cell: true,
      ..Self::default()
    }
  }

  fn visit_asciidoc_table_cell_result(&mut self, result: Result<Self::Output, Self::Error>) {
    self.html.push_str(&result.unwrap());
  }

  fn enter_table_section(&mut self, section: TableSection) {
    match section {
      TableSection::Header => self.push_str("<thead>"),
      TableSection::Body => self.push_str("<tbody>"),
      TableSection::Footer => self.push_str("<tfoot>"),
    }
  }

  fn exit_table_section(&mut self, section: TableSection) {
    match section {
      TableSection::Header => self.push_str("</thead>"),
      TableSection::Body => self.push_str("</tbody>"),
      TableSection::Footer => self.push_str("</tfoot>"),
    }
  }

  fn enter_table_row(&mut self, _row: &Row, _section: TableSection) {
    self.push_str("<tr>");
  }

  fn exit_table_row(&mut self, _row: &Row, _section: TableSection) {
    self.push_str("</tr>");
  }

  fn enter_table_cell(&mut self, cell: &Cell, section: TableSection) {
    self.open_cell(cell, section);
  }

  fn exit_table_cell(&mut self, cell: &Cell, section: TableSection) {
    self.close_cell(cell, section);
  }

  fn enter_cell_paragraph(&mut self, cell: &Cell, section: TableSection) {
    self.open_cell_paragraph(cell, section);
  }

  fn exit_cell_paragraph(&mut self, cell: &Cell, section: TableSection) {
    self.close_cell_paragraph(cell, section);
  }

  fn enter_inline_italic(&mut self, _children: &[InlineNode]) {
    self.push_str("<em>");
  }

  fn exit_inline_italic(&mut self, _children: &[InlineNode]) {
    self.push_str("</em>");
  }

  fn visit_thematic_break(&mut self, block: &Block) {
    self.open_element("hr", &[], block.meta.attrs.as_ref());
  }

  fn visit_page_break(&mut self, _block: &Block) {
    self.push_str(r#"<div style="page-break-after: always;"></div>"#);
  }

  fn visit_inline_text(&mut self, text: &str) {
    self.push_str(text);
  }

  fn visit_joining_newline(&mut self) {
    match self.newlines {
      Newlines::JoinWithSpace => self.push_ch(' '),
      Newlines::JoinWithBreak => self.push_str("<br> "),
      Newlines::Preserve => self.push_str("\n"),
    }
  }

  fn enter_text_span(&mut self, attrs: &AttrList, _children: &[InlineNode]) {
    self.open_element("span", &[], Some(attrs));
  }

  fn exit_text_span(&mut self, _attrs: &AttrList, _children: &[InlineNode]) {
    self.push_str("</span>");
  }

  fn enter_xref(&mut self, id: &str, _target: Option<&[InlineNode]>) {
    self.push(["<a href=\"#", id, "\">"]);
  }

  fn exit_xref(&mut self, _id: &str, _target: Option<&[InlineNode]>) {
    self.push_str("</a>");
  }

  fn visit_missing_xref(&mut self, id: &str) {
    self.push(["[", id, "]"]);
  }

  fn visit_inline_anchor(&mut self, id: &str) {
    self.push(["<a id=\"", id, "\"></a>"]);
  }

  fn visit_callout(&mut self, callout: Callout) {
    if !self.html.ends_with(' ') {
      self.push_ch(' ');
    }
    match self.doc_meta.icon_mode() {
      IconMode::Image => self.push_callout_number_img(callout.number),
      IconMode::Font => self.push_callout_number_font(callout.number),
      // TODO: asciidoctor also handles special `guard` case
      //   elsif ::Array === (guard = node.attributes['guard'])
      //     %(&lt;!--<b class="conum">(#{node.text})</b>--&gt;)
      // @see https://github.com/asciidoctor/asciidoctor/issues/3319
      IconMode::Text => self.push([r#"<b class="conum">("#, &num_str!(callout.number), ")</b>"]),
    }
  }

  fn visit_callout_tuck(&mut self, comment: &str) {
    if self.doc_meta.icon_mode() != IconMode::Font {
      self.push_str(comment);
    }
  }

  fn visit_linebreak(&mut self) {
    self.push_str("<br> ");
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

  fn visit_button_macro(&mut self, text: &str) {
    self.push([r#"<b class="button">"#, text, "</b>"])
  }

  fn visit_image_macro(&mut self, target: &str, attrs: &AttrList) {
    let mut open_tag = OpenTag::new("span", None);
    open_tag.push_class("image");
    open_tag.push_opt_class(attrs.named("float"));
    open_tag.push_opt_prefixed_class(attrs.named("align"), Some("text-"));
    open_tag.push_classes(attrs.roles.iter());
    self.push_open_tag(open_tag);

    let with_link = if let Some(link_href) = attrs.named("link") {
      let mut a_tag = OpenTag::new("a", None);
      a_tag.push_class("image");
      a_tag.push_str("\" href=\"");
      if link_href == "self" {
        push_img_path(a_tag.htmlbuf(), target, &self.doc_meta);
      } else {
        a_tag.push_str_attr_escaped(link_href);
      }
      a_tag.push_link_attrs(attrs, true, false);
      self.push_open_tag(a_tag);
      true
    } else {
      false
    };

    self.render_image(target, attrs);
    if with_link {
      self.push_str("</a>");
    }
    self.push_str("</span>");
  }

  fn visit_keyboard_macro(&mut self, keys: &[&str]) {
    if keys.len() > 1 {
      self.push_str(r#"<span class="keyseq">"#);
    }
    for (idx, key) in keys.iter().enumerate() {
      if idx > 0 {
        self.push_ch('+');
      }
      self.push(["<kbd>", key, "</kbd>"]);
    }
    if keys.len() > 1 {
      self.push_str("</span>");
    }
  }

  fn enter_link_macro(
    &mut self,
    target: &str,
    attrs: Option<&AttrList>,
    scheme: Option<UrlScheme>,
    has_link_text: bool,
    blank_window_shorthand: bool,
  ) {
    let mut tag = OpenTag::new("a", None);
    tag.push_str(" href=\"");
    if matches!(scheme, Some(UrlScheme::Mailto)) {
      tag.push_str("mailto:");
    }
    tag.push_str(target);
    tag.push_ch('"');

    if let Some(attrs) = attrs {
      tag.push_link_attrs(attrs, has_link_text, blank_window_shorthand);
    }

    if attrs.is_none() && (!has_link_text && !matches!(scheme, Some(UrlScheme::Mailto))) {
      tag.push_class("bare")
    }

    self.push_open_tag(tag);
  }

  fn exit_link_macro(
    &mut self,
    target: &str,
    _attrs: Option<&AttrList>,
    _scheme: Option<UrlScheme>,
    has_link_text: bool,
  ) {
    if has_link_text {
      self.push_str("</a>");
      return;
    }
    if self.doc_meta.is_true("hide-uri-scheme") {
      self.push_str(str_util::remove_uri_scheme(target));
    } else {
      self.push_str(target);
    }
    self.push_str("</a>");
  }

  fn visit_menu_macro(&mut self, items: &[&str]) {
    let mut items = items.iter();
    self.push_str(r#"<span class="menuseq"><span class="menu">"#);
    self.push_str(items.next().unwrap());
    self.push_str("</span>");

    let last_idx = items.len() - 1;
    for (idx, item) in items.enumerate() {
      self.push_str(r#"&#160;&#9656;<span class=""#);
      if idx == last_idx {
        self.push(["menuitem\">", item, "</span>"]);
      } else {
        self.push(["submenu\">", item, "</span>"]);
      }
    }
    self.push_str("</span>");
  }

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

  fn enter_inline_subscript(&mut self, _children: &[InlineNode]) {
    self.push_str("<sub>");
  }

  fn exit_inline_subscript(&mut self, _children: &[InlineNode]) {
    self.push_str("</sub>");
  }

  fn enter_inline_superscript(&mut self, _children: &[InlineNode]) {
    self.push_str("<sup>");
  }

  fn exit_inline_superscript(&mut self, _children: &[InlineNode]) {
    self.push_str("</sup>");
  }

  fn enter_inline_quote(&mut self, kind: QuoteKind, _children: &[InlineNode]) {
    match kind {
      QuoteKind::Double => self.push_str("&#8220;"),
      QuoteKind::Single => self.push_str("&#8216;"),
    }
  }

  fn exit_inline_quote(&mut self, kind: QuoteKind, _children: &[InlineNode]) {
    match kind {
      QuoteKind::Double => self.push_str("&#8221;"),
      QuoteKind::Single => self.push_str("&#8217;"),
    }
  }

  fn visit_curly_quote(&mut self, kind: CurlyKind) {
    match kind {
      CurlyKind::LeftDouble => self.push_str("&#8221;"),
      CurlyKind::RightDouble => self.push_str("&#8220;"),
      CurlyKind::LeftSingle => self.push_str("&#8217;"),
      CurlyKind::RightSingle => self.push_str("&#8216;"),
      CurlyKind::LegacyImplicitApostrophe => self.push_str("&#8217;"),
    }
  }

  fn visit_inline_lit_mono(&mut self, text: &str) {
    self.push(["<code>", text, "</code>"]);
  }

  fn visit_multichar_whitespace(&mut self, _whitespace: &str) {
    self.push_ch(' ');
  }

  fn enter_admonition_block(&mut self, kind: AdmonitionKind, block: &Block) {
    let classes = &["admonitionblock", kind.lowercase_str()];
    self.open_element("div", classes, block.meta.attrs.as_ref());
    self.push_str(r#"<table><tr><td class="icon">"#);
    match self.doc_meta.icon_mode() {
      IconMode::Text => {
        self.push([r#"<div class="title">"#, kind.str()]);
        self.push_str(r#"</div></td><td class="content">"#);
      }
      IconMode::Image => {
        self.push_admonition_img(kind);
        self.push_str(r#"</td><td class="content">"#);
      }
      IconMode::Font => {
        self.push([r#"<i class="fa icon-"#, kind.lowercase_str(), "\" title=\""]);
        self.push([kind.str(), r#""></i></td><td class="content">"#]);
      }
    }
    self.render_block_title(&block.meta);
  }

  fn exit_admonition_block(&mut self, _kind: AdmonitionKind, _block: &Block) {
    self.push_str(r#"</td></tr></table></div>"#);
  }

  fn enter_image_block(&mut self, img_target: &str, img_attrs: &AttrList, block: &Block) {
    let mut open_tag = OpenTag::new("div", block.meta.attrs.as_ref());
    open_tag.push_class("imageblock");
    open_tag.push_opt_class(img_attrs.named("float"));
    open_tag.push_opt_prefixed_class(img_attrs.named("align"), Some("text-"));
    self.push_open_tag(open_tag);

    self.push_str(r#"<div class="content">"#);
    let mut has_link = false;
    if let Some(href) = &block.named_attr("link").or_else(|| img_attrs.named("link")) {
      self.push([r#"<a class="image" href=""#, *href, r#"">"#]);
      has_link = true;
    }
    self.render_image(img_target, img_attrs);
    if has_link {
      self.push_str("</a>");
    }
    self.push_str(r#"</div>"#);
  }

  fn exit_image_block(&mut self, block: &Block) {
    let prefix = if self.doc_meta.is_false("figure-caption") {
      None
    } else {
      self.fig_caption_num += 1;
      Some(Cow::Owned(format!("Figure {}. ", self.fig_caption_num)))
    };
    self.render_prefixed_block_title(&block.meta, prefix);
    self.push_str(r#"</div>"#);
  }

  fn visit_document_attribute_decl(&mut self, name: &str, value: &AttrValue) {
    if name == "hardbreaks-option" {
      if value.is_true() {
        self.default_newlines = Newlines::JoinWithBreak;
        self.newlines = Newlines::JoinWithBreak;
      } else {
        self.default_newlines = Newlines::default();
        self.newlines = Newlines::default();
      }
    }
    // TODO: consider warning?
    _ = self.doc_meta.insert_doc_attr(name, value.clone());
  }

  fn enter_footnote(&mut self, _num: u16, _id: Option<&str>, _content: &[InlineNode]) {
    self.start_buffering();
  }

  fn exit_footnote(&mut self, num: u16, id: Option<&str>, _content: &[InlineNode]) {
    let footnote = self.take_buffer();
    let nums = num.to_string();
    self.push_str(r#"<sup class="footnote""#);
    if let Some(id) = id {
      self.push([r#" id="_footnote_"#, id, "\""]);
    }
    self.push_str(r#">[<a id="_footnoteref_"#);
    self.push([&nums, r##"" class="footnote" href="#_footnotedef_"##, &nums]);
    self.push([r#"" title="View footnote.">"#, &nums, "</a>]</sup>"]);
    let id = id.unwrap_or(&nums);
    self.footnotes.push((num, id.to_string(), footnote));
  }

  fn into_result(self) -> Result<Self::Output, Self::Error> {
    Ok(self.html)
  }

  fn result(&self) -> Result<&Self::Output, Self::Error> {
    Ok(&self.html)
  }
}

impl HtmlBuf for AsciidoctorHtml {
  fn htmlbuf(&mut self) -> &mut String {
    &mut self.html
  }
}

impl AsciidoctorHtml {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn into_string(self) -> String {
    self.html
  }

  pub(crate) fn push_buffered(&mut self) {
    let mut buffer = String::new();
    mem::swap(&mut buffer, &mut self.alt_html);
    self.push_str(&buffer);
  }

  pub(crate) fn push_open_tag(&mut self, tag: OpenTag) {
    self.push_str(&tag.finish());
  }

  fn source_lang<'a>(&self, block: &'a Block) -> Option<Cow<'a, str>> {
    match block
      .meta
      .attrs
      .as_ref()
      .map(|a| (a.str_positional_at(0), a.str_positional_at(1)))
      .unwrap_or((None, None))
    {
      (None | Some("source"), Some(lang)) => Some(Cow::Borrowed(lang)),
      _ => self
        .doc_meta
        .str("source-language")
        .map(|s| Cow::Owned(s.to_string())),
    }
  }

  fn render_block_title(&mut self, meta: &ChunkMeta) {
    if meta.title.is_some() {
      self.push_str(r#"<div class="title">"#);
      self.push_buffered();
      self.push_str("</div>");
    }
  }

  fn render_prefixed_block_title(&mut self, meta: &ChunkMeta, prefix: Option<Cow<str>>) {
    if meta.title.is_some() {
      self.push_str(r#"<div class="title">"#);
      if let Some(prefix) = prefix {
        self.push_str(&prefix);
      }
      self.push_buffered();
      self.push_str("</div>");
    }
  }

  pub(crate) fn open_element(&mut self, element: &str, classes: &[&str], attrs: Option<&AttrList>) {
    let mut open_tag = OpenTag::new(element, attrs);
    classes.iter().for_each(|c| open_tag.push_class(c));
    self.push_open_tag(open_tag);
  }

  fn render_footnotes(&mut self) {
    self.push_str(r#"<div id="footnotes"><hr>"#);
    let footnotes = mem::take(&mut self.footnotes);
    for (num, _id, footnote) in &footnotes {
      let num = num.to_string();
      self.push_str(r#"<div class="footnote" id="_footnotedef_"#);
      self.push([&num, r##""><a href="#_footnoteref_"##, &num, "\">"]);
      self.push([&num, "</a>. ", footnote, "</div>"]);
    }
    self.push_str(r#"</div>"#);
    self.footnotes = footnotes;
  }

  fn render_favicon(&mut self, meta: &DocumentMeta) {
    match meta.get("favicon") {
      Some(AttrValue::String(path)) => {
        let ext = helpers::file_ext(path).unwrap_or("ico");
        self.push_str(r#"<link rel="icon" type="image/"#);
        self.push([ext, r#"" href=""#, &path, "\">"]);
      }
      Some(AttrValue::Bool(true)) => {
        self.push_str(r#"<link rel="icon" type="image/x-icon" href="favicon.ico">"#);
      }
      _ => {}
    }
  }

  fn render_authors(&mut self, authors: &[Author]) {
    if authors.is_empty() {
      return;
    }
    self.push_str(r#"<meta name="author" content=""#);
    for (index, author) in authors.iter().enumerate() {
      if index > 0 {
        self.push_str(", ");
      }
      // TODO: escape/sanitize, w/ tests, see asciidoctor
      self.push_str(&author.fullname());
    }
    self.push_str(r#"">"#);
  }

  fn render_title(&mut self, document: &Document, attrs: &DocumentMeta) {
    self.push_str(r#"<title>"#);
    if let Some(title) = attrs.str("title") {
      self.push_str(title);
    } else if let Some(title) = document.title.as_ref() {
      for s in title.plain_text() {
        self.push_str(s);
      }
    } else {
      self.push_str("Untitled");
    }
    self.push_str(r#"</title>"#);
  }

  fn exit_attributed(
    &mut self,
    context: BlockContext,
    attribution: Option<&str>,
    cite: Option<&str>,
  ) {
    if context == BlockContext::BlockQuote {
      self.push_str("</blockquote>");
    } else {
      self.push_str("</pre>");
    }
    if let Some(attribution) = attribution {
      self.push_str(r#"<div class="attribution">&#8212; "#);
      self.push_str(attribution);
      if let Some(cite) = cite {
        self.push_str(r#"<br><cite>"#);
        self.push([cite, "</cite>"]);
      }
      self.push_str("</div>");
    } else if let Some(cite) = cite {
      self.push_str(r#"<div class="attribution">&#8212; "#);
      self.push([cite, "</div>"]);
    }
    self.push_str("</div>");
  }

  fn render_checklist_item(&mut self, item: &ListItem) {
    if let ListItemTypeMeta::Checklist(checked, _) = &item.type_meta {
      match (self.list_stack.last() == Some(&true), checked) {
        (false, true) => self.push_str("&#10003;"),
        (false, false) => self.push_str("&#10063;"),
        (true, true) => self.push_str(r#"<input type="checkbox" data-item-complete="1" checked>"#),
        (true, false) => self.push_str(r#"<input type="checkbox" data-item-complete="0">"#),
      }
    }
  }

  fn start_buffering(&mut self) {
    mem::swap(&mut self.html, &mut self.alt_html);
  }

  fn stop_buffering(&mut self) {
    mem::swap(&mut self.html, &mut self.alt_html);
  }

  fn take_buffer(&mut self) -> String {
    mem::swap(&mut self.alt_html, &mut self.html);
    let mut buffered = String::new();
    mem::swap(&mut buffered, &mut self.alt_html);
    buffered
  }

  // TODO: handle embedding images, data-uri, etc., this is a naive impl
  // @see https://github.com/jaredh159/asciidork/issues/7
  fn push_icon_uri(&mut self, name: &str, prefix: Option<&str>) {
    // PERF: we could work to prevent all these allocations w/ some caching
    // these might get rendered many times in a given document
    let icondir = self.doc_meta.string_or("iconsdir", "./images/icons");
    let ext = self.doc_meta.string_or("icontype", "png");
    self.push([&icondir, "/", prefix.unwrap_or(""), name, ".", &ext]);
  }

  fn push_admonition_img(&mut self, kind: AdmonitionKind) {
    self.push_str(r#"<img src=""#);
    self.push_icon_uri(kind.lowercase_str(), None);
    self.push([r#"" alt=""#, kind.str(), r#"">"#]);
  }

  fn push_callout_number_img(&mut self, num: u8) {
    let n_str = &num_str!(num);
    self.push_str(r#"<img src=""#);
    self.push_icon_uri(n_str, Some("callouts/"));
    self.push([r#"" alt=""#, n_str, r#"">"#]);
  }

  fn push_callout_number_font(&mut self, num: u8) {
    let n_str = &num_str!(num);
    self.push([r#"<i class="conum" data-value=""#, n_str, r#""></i>"#]);
    self.push([r#"<b>("#, n_str, ")</b>"]);
  }

  fn render_document_authors(&mut self) {
    let authors = self.doc_meta.authors();
    if self.doc_meta.embedded || authors.is_empty() {
      return;
    }
    let mut buffer = String::with_capacity(authors.len() * 100);
    buffer.push_str(r#"<div class="details">"#);
    for (idx, author) in authors.iter().enumerate() {
      buffer.push_str(r#"<span id="author"#);
      if idx > 0 {
        buffer.push_str(&num_str!(idx + 1));
      }
      buffer.push_str(r#"" class="author">"#);
      buffer.push_str(&author.fullname());
      buffer.push_str(r#"</span><br>"#);
      if let Some(email) = &author.email {
        buffer.push_str(r#"<span id="email"#);
        if idx > 0 {
          buffer.push_str(&num_str!(idx + 1));
        }
        buffer.push_str(r#"" class="email"><a href="mailto:"#);
        buffer.push_str(email);
        buffer.push_str(r#"">"#);
        buffer.push_str(email);
        buffer.push_str(r#"</a></span><br>"#);
      }
    }
    self.push([&buffer, "</div>"]);
  }

  fn standalone(&self) -> bool {
    self.doc_meta.get_doctype() != DocType::Inline
      && !self.in_asciidoc_table_cell
      && !self.doc_meta.embedded
  }

  fn render_doc_title(&self) -> bool {
    if self.doc_meta.is_true("noheader")
      || self.doc_meta.is_true("notitle")
      || self.doc_meta.is_false("showtitle")
      || (self.doc_meta.embedded && !self.doc_meta.is_true("showtitle"))
    {
      return false;
    }
    true
  }

  fn render_interactive_svg(&mut self, target: &str, attrs: &AttrList) {
    self.push_str(r#"<object type="image/svg+xml" data=""#);
    push_img_path(&mut self.html, target, &self.doc_meta);
    self.push_ch('"');
    self.push_named_or_pos_attr("width", 1, attrs);
    self.push_named_or_pos_attr("height", 2, attrs);
    self.push_ch('>');
    if let Some(fallback) = attrs.named("fallback") {
      self.push_str(r#"<img src=""#);
      push_img_path(&mut self.html, fallback, &self.doc_meta);
      self.push_ch('"');
      self.push_named_or_pos_attr("alt", 0, attrs);
      self.push_ch('>');
    } else if let Some(alt) = attrs.named("alt").or_else(|| attrs.str_positional_at(0)) {
      self.push([r#"<span class="alt">"#, alt, "</span>"]);
    }
    self.push_str("</object>");
  }

  fn render_image(&mut self, target: &str, attrs: &AttrList) {
    let format = attrs.named("format").or_else(|| str_util::file_ext(target));
    let is_svg = matches!(format, Some("svg" | "SVG"));
    if is_svg && attrs.has_option("interactive") && self.doc_meta.safe_mode != SafeMode::Secure {
      return self.render_interactive_svg(target, attrs);
    }
    self.push_str(r#"<img src=""#);
    push_img_path(&mut self.html, target, &self.doc_meta);
    self.push_str(r#"" alt=""#);
    if let Some(alt) = attrs.named("alt").or_else(|| attrs.str_positional_at(0)) {
      self.push_str_attr_escaped(alt);
    } else if let Some(Some(nodes)) = attrs.positional.first() {
      for s in nodes.plain_text() {
        self.push_str_attr_escaped(s);
      }
    } else {
      let alt = str_util::filestem(target).replace(['-', '_'], " ");
      self.push_str_attr_escaped(&alt);
    }
    self.push_ch('"');
    self.push_named_or_pos_attr("width", 1, attrs);
    self.push_named_or_pos_attr("height", 2, attrs);
    self.push_named_attr("title", attrs);
    self.push_ch('>');
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Newlines {
  JoinWithBreak,
  #[default]
  JoinWithSpace,
  Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EphemeralState {
  VisitingSimpleTermDescription,
  IsSourceBlock,
}

const fn list_type_from_depth(depth: u8) -> &'static str {
  match depth {
    1 => "1",
    2 => "a",
    3 => "i",
    4 => "A",
    _ => "I",
  }
}

fn list_type_from_class(class: &str) -> Option<&'static str> {
  match class {
    "arabic" => Some("1"),
    "loweralpha" => Some("a"),
    "lowerroman" => Some("i"),
    "upperalpha" => Some("A"),
    "upperroman" => Some("I"),
    _ => None,
  }
}

const fn list_class_from_depth(depth: u8) -> &'static str {
  match depth {
    1 => "arabic",
    2 => "loweralpha",
    3 => "lowerroman",
    4 => "upperalpha",
    _ => "upperroman",
  }
}

macro_rules! num_str {
  ($n:expr) => {
    match $n {
      0 => Cow::Borrowed("0"),
      1 => Cow::Borrowed("1"),
      2 => Cow::Borrowed("2"),
      3 => Cow::Borrowed("3"),
      4 => Cow::Borrowed("4"),
      5 => Cow::Borrowed("5"),
      6 => Cow::Borrowed("6"),
      _ => Cow::Owned($n.to_string()),
    }
  };
}

pub(crate) use num_str;

lazy_static! {
  pub static ref REMOVE_FILE_EXT: Regex = Regex::new(r"^(.*)\.[^.]+$").unwrap();
}
