use crate::web;
use ammonia::Builder;
use pulldown_cmark::{CowStr, Event, LinkType, Options, Parser, Tag, TagEnd, TextMergeStream, html};
use std::borrow::Cow;
use std::fmt::Write;
use std::ops::Range;
use std::sync::LazyLock;
use url::Url;

pub fn render(text: &str) -> String {
    let (text, sjis_art) = mark_sjis(text);
    let text = rewrite_source(&text);
    let parser = TextMergeStream::new(Parser::new_ext(&text, OPTIONS));
    let renderer = Renderer {
        sjis_art,
        ..Renderer::default()
    };
    let mut html = String::new();
    html::push_html(&mut html, renderer.run(parser).into_iter());
    SANITIZER.clean(&html).to_string()
}

const SJIS_OPEN: &str = "[sjis]";
const SJIS_CLOSE: &str = "[/sjis]";
const MARKER_START: char = '\u{E000}';
const MARKER_END: char = '\u{E001}';

const OPTIONS: Options = Options::ENABLE_STRIKETHROUGH
    .union(Options::ENABLE_TABLES)
    .union(Options::ENABLE_SMART_PUNCTUATION);

static SANITIZER: LazyLock<Builder<'static>> = LazyLock::new(|| {
    let mut builder = Builder::default();
    builder
        // These pick up site styling by tag name, which lets users forge comment headers
        .rm_tags(&["header", "nav", "footer", "aside", "article"])
        .add_generic_attributes(&["style"])
        .add_allowed_classes("span", &["spoiler", "sjis"])
        // External resources would reveal viewers' IPs to whoever hosts them. The Content-Security-Policy
        // header also blocks them, so this mainly keeps them from showing up as broken images.
        .attribute_filter(|element, attribute, value| match (element, attribute) {
            ("img", "src") if !is_same_origin(value) => None,
            (_, "style") if loads_resources(value) => None,
            _ => Some(value.into()),
        });
    builder
});

struct SjisArt<'a> {
    source: &'a str,
    /// [`None`] indicates that art has already been used, so markers the parser duplicates can't multiply it
    html: Option<String>,
}

struct SjisMatch {
    start: usize,
    end: usize,
}

impl SjisMatch {
    fn content_range(&self) -> Range<usize> {
        let content_start = self.start + SJIS_OPEN.len();
        let content_end = self.end - SJIS_CLOSE.len();
        content_start..content_end
    }
}

fn is_same_origin(url: &str) -> bool {
    static BASE: LazyLock<Url> = LazyLock::new(|| Url::parse("http://booru.invalid/").unwrap());
    BASE.join(url).is_ok_and(|resolved| resolved.origin() == BASE.origin())
}

// Backslashes are rejected too, since CSS escapes could spell out these functions
fn loads_resources(style: &str) -> bool {
    let style = style.to_ascii_lowercase();
    style.contains('\\')
        || ["url(", "image(", "image-set(", "src("]
            .iter()
            .any(|function| style.contains(function))
}

/// Swaps SJIS blocks in `text` for a marker so Markdown keeps its whitespace intact. The renderer
/// then turns markers back into art. Code, links, and raw HTML are exceptions to this, where the
/// source text is restored as-is.
fn mark_sjis(text: &str) -> (Cow<'_, str>, Vec<SjisArt<'_>>) {
    if !contains_ignore_ascii_case(text, SJIS_OPEN) {
        return (Cow::Borrowed(text), Vec::new());
    }

    let lowercase = text.to_ascii_lowercase();
    let mut position = 0;
    let mut sjis_art = Vec::new();
    let mut marked = String::with_capacity(text.len());
    while let Some(sjis) = find_sjis(&lowercase, position) {
        push_without_markers(&mut marked, &text[position..sjis.start]);
        write!(marked, "{MARKER_START}{}{MARKER_END}", sjis_art.len()).expect("Write to String is infallible");

        // Lines holding only the tags aren't part of the art, so they don't add blank lines
        let mut art = &text[sjis.content_range()];
        if let Some((first_line, rest)) = art.split_once('\n')
            && first_line.trim().is_empty()
        {
            art = rest;
        }
        if let Some((rest, last_line)) = art.rsplit_once('\n')
            && last_line.trim().is_empty()
        {
            art = rest.trim_suffix('\r');
        }

        let art = ammonia::clean_text(art);
        sjis_art.push(SjisArt {
            source: &text[sjis.start..sjis.end],
            html: Some(format!(r#"<span class="sjis">{art}</span>"#)),
        });
        position = sjis.end;
    }
    push_without_markers(&mut marked, &text[position..]);
    (Cow::Owned(marked), sjis_art)
}

fn find_sjis(lowercase: &str, mut position: usize) -> Option<SjisMatch> {
    loop {
        let start = position + lowercase[position..].find(SJIS_OPEN)?;
        let content_start = start + SJIS_OPEN.len();
        let content_end = content_start + lowercase[content_start..].find(SJIS_CLOSE)?;
        let end = content_end + SJIS_CLOSE.len();
        match lowercase[content_start..content_end].rfind(SJIS_OPEN) {
            Some(inner) => position = content_start + inner,
            None if content_end > content_start => return Some(SjisMatch { start, end }),
            None => position = start + 1, // Skip empty SJIS blocks
        }
    }
}

/// Appends `text` to `marked` with user-typed marker characters replaced.
/// This is done to ensure that every marker refers to art exactly once.
fn push_without_markers(marked: &mut String, text: &str) {
    for (index, part) in text.split([MARKER_START, MARKER_END]).enumerate() {
        if index > 0 {
            marked.push(char::REPLACEMENT_CHARACTER);
        }
        marked.push_str(part);
    }
}

// Returns the art index and the marker's length
fn parse_marker(text: &str) -> Option<(usize, usize)> {
    let digits = text.strip_prefix(MARKER_START)?;
    let digits_len = digits.bytes().take_while(u8::is_ascii_digit).count();
    let index = digits[..digits_len].parse().ok()?;
    digits[digits_len..]
        .starts_with(MARKER_END)
        .then_some((index, MARKER_START.len_utf8() + digits_len + MARKER_END.len_utf8()))
}

/// Rewrites `text` so that bare URLs and icon blocks become escaped Markdown links before parsing, so emphasis and
/// smart punctuation can't alter them.
fn rewrite_source(text: &str) -> Cow<'_, str> {
    if !contains_ignore_ascii_case(text, "http") && !contains_ignore_ascii_case(text, "www.") {
        return Cow::Borrowed(text);
    }

    let (verbatim_ranges, cell_ends) = scan_structure(text);
    let mut verbatim_ranges = verbatim_ranges.iter().peekable();
    let mut cell_ends = cell_ends.iter().peekable();
    let mut linked = String::with_capacity(text.len());
    let mut copied = 0;
    let mut position = 0;
    while position < text.len() {
        while verbatim_ranges.next_if(|range| range.end <= position).is_some() {}
        while cell_ends.next_if(|&&end| end <= position).is_some() {}
        let cell_end = cell_ends.peek().map_or(text.len(), |&&end| end);
        let limit = match verbatim_ranges.peek() {
            Some(range) if range.start <= position => {
                position = range.end;
                continue;
            }
            Some(range) => range.start.min(cell_end),
            None => cell_end,
        };

        let rest = &text[position..limit];
        if let Some((icon_len, url)) = icon(rest) {
            linked.push_str(&text[copied..position]);
            push_url_link(&mut linked, url);
            position += icon_len;
            copied = position;
        } else if let Some(url) = bare_url(rest) {
            linked.push_str(&text[copied..position]);
            push_url_link(&mut linked, url);
            position += url.len();
            copied = position;
        } else {
            position += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    linked.push_str(&text[copied..]);
    Cow::Owned(linked)
}

/// Finds source ranges that aren't prose (code, links, raw HTML, and link reference definitions)
/// and where table cells end, since cells are the only structure a URL could run into within a line.
fn scan_structure(text: &str) -> (Vec<Range<usize>>, Vec<usize>) {
    let parser = Parser::new_ext(text, OPTIONS);

    let mut cell_ends = Vec::new();
    let mut ranges: Vec<_> = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| definition.span.clone())
        .collect();
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::TableCell) => cell_ends.push(range.end),
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::Link { .. } | Tag::Image { .. })
            | Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_) => ranges.push(range),
            _ => (),
        }
    }
    ranges.sort_unstable_by_key(|range| range.start);
    (ranges, cell_ends)
}

fn push_url_link(markdown: &mut String, url: &str) {
    markdown.push('[');
    push_escaped_markdown(markdown, url);
    markdown.push_str("](<");
    push_escaped_markdown(markdown, &dest_url(url));
    markdown.push_str(">)");
}

fn dest_url(url: &str) -> Cow<'_, str> {
    if strip_prefix_ignore_ascii_case(url, "www.").is_some() {
        Cow::Owned(format!("http://{url}"))
    } else {
        Cow::Borrowed(url)
    }
}

fn push_escaped_markdown(markdown: &mut String, text: &str) {
    for c in text.chars() {
        if c.is_ascii_punctuation() {
            markdown.push('\\');
        }
        markdown.push(c);
    }
}

const SPOILER_OPEN: &str = "[spoiler]";
const SPOILER_CLOSE: &str = "[/spoiler]";
const ICON_OPEN: &str = "[icon]";
const ICON_CLOSE: &str = "[/icon]";
const URL_PREFIXES: [&str; 3] = ["https://", "http://", "www."];

// Returns whether the tag closes and its length
fn spoiler_tag(text: &str) -> Option<(bool, usize)> {
    if strip_prefix_ignore_ascii_case(text, SPOILER_OPEN).is_some() {
        Some((false, SPOILER_OPEN.len()))
    } else if strip_prefix_ignore_ascii_case(text, SPOILER_CLOSE).is_some() {
        Some((true, SPOILER_CLOSE.len()))
    } else {
        None
    }
}

// Returns the length of `[icon]url[/icon]` and its URL
fn icon(text: &str) -> Option<(usize, &str)> {
    let content = strip_prefix_ignore_ascii_case(text, ICON_OPEN)?;
    let url_len = content
        .find(|c: char| c.is_whitespace() || matches!(c, '[' | '<' | MARKER_START))
        .unwrap_or(content.len());
    let url = &content[..url_len];
    strip_prefix_ignore_ascii_case(&content[url_len..], ICON_CLOSE)?;
    URL_PREFIXES
        .iter()
        .any(|prefix| url.len() > prefix.len() && strip_prefix_ignore_ascii_case(url, prefix).is_some())
        .then_some((ICON_OPEN.len() + url_len + ICON_CLOSE.len(), url))
}

fn starts_with_tag(text: &str) -> bool {
    [SPOILER_OPEN, SPOILER_CLOSE, ICON_OPEN, ICON_CLOSE]
        .iter()
        .any(|tag| strip_prefix_ignore_ascii_case(text, tag).is_some())
}

fn contains_ignore_ascii_case(text: &str, needle: &str) -> bool {
    text.as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn strip_prefix_ignore_ascii_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len())
        .filter(|start| start.eq_ignore_ascii_case(prefix))
        .map(|_| &text[prefix.len()..])
}

struct OpenSpoiler {
    index: usize,
    depth: usize,
}

#[derive(Default)]
struct Renderer<'a> {
    events: Vec<Event<'a>>,
    open_spoilers: Vec<OpenSpoiler>,
    sjis_art: Vec<SjisArt<'a>>,
    depth: usize,
    in_link: bool,
    in_code_block: bool,
}

impl<'a> Renderer<'a> {
    fn run(mut self, parser: impl Iterator<Item = Event<'a>>) -> Vec<Event<'a>> {
        let mut image: Option<Image> = None;
        let mut images_in_alt = 0;
        for event in parser {
            // Art outside of prose is shown as its source text
            let event = match event {
                Event::Code(text) => Event::Code(self.unmark(text)),
                Event::InlineHtml(html) => Event::InlineHtml(self.unmark(html)),
                Event::Html(html) => Event::Html(self.unmark(html)),
                Event::Text(text) if self.in_code_block || image.is_some() => Event::Text(self.unmark(text)),
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }) => Event::Start(Tag::Link {
                    link_type,
                    dest_url: self.unmark(dest_url),
                    title: self.unmark(title),
                    id,
                }),
                Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                }) => Event::Start(Tag::Image {
                    link_type,
                    dest_url: self.unmark(dest_url),
                    title,
                    id,
                }),
                event => event,
            };

            // Images are rendered once their alt text is known
            if let Some(open_image) = &mut image {
                match event {
                    Event::Start(Tag::Image { .. }) => images_in_alt += 1,
                    Event::End(TagEnd::Image) if images_in_alt > 0 => images_in_alt -= 1,
                    Event::End(TagEnd::Image) => {
                        if let Some(image) = image.take() {
                            self.push_image(image);
                        }
                    }
                    Event::Text(text) | Event::Code(text) => open_image.alt.push_str(&text),
                    _ => (),
                }
                continue;
            }

            match event {
                Event::Start(Tag::Image { dest_url, .. }) => image = Some(Image::parse(&dest_url)),
                Event::Start(ref tag) => {
                    match tag {
                        Tag::Link { .. } => self.in_link = true,
                        Tag::CodeBlock(_) => self.in_code_block = true,
                        _ => (),
                    }
                    self.depth += 1;
                    self.events.push(event);
                }
                Event::End(tag_end) => {
                    // Spoilers left open when their container ends stay as plain text
                    while self.open_spoilers.last().is_some_and(|open| open.depth >= self.depth) {
                        self.open_spoilers.pop();
                    }
                    match tag_end {
                        TagEnd::Link => self.in_link = false,
                        TagEnd::CodeBlock => self.in_code_block = false,
                        _ => (),
                    }
                    self.depth -= 1;
                    // Art on its own isn't wrapped in a paragraph, like in the original client
                    if let [.., Event::Start(Tag::Paragraph), Event::Html(_)] = self.events[..]
                        && tag_end == TagEnd::Paragraph
                    {
                        let art = self.events.pop();
                        self.events.pop();
                        self.events.extend(art);
                    } else {
                        self.events.push(event);
                    }
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !matches!(self.events.last(), Some(Event::Html(_))) {
                        self.events.push(Event::HardBreak);
                    }
                }
                Event::Text(text) if !self.in_code_block => self.push_text(&text),
                _ => self.events.push(event),
            }
        }
        self.events
    }

    fn push_image(&mut self, image: Image) {
        if is_same_origin(&image.url) {
            return self.events.push(Event::InlineHtml(image.to_html().into()));
        }

        // External images would reveal viewers' IPs to whoever hosts them, so they become links,
        // or just text inside another link since links can't nest
        let Image { url, alt, .. } = image;
        let text = if alt.is_empty() { url.clone() } else { alt };
        if self.in_link {
            self.push_plain(&text);
        } else {
            self.push_link(url, &text);
        }
    }

    fn push_text(&mut self, text: &str) {
        let mut plain_start = 0;
        let mut position = 0;
        while let Some(next_char) = text[position..].chars().next() {
            let rest = &text[position..];
            let token_len = if let Some((closing, len)) = spoiler_tag(rest) {
                self.push_plain(&text[plain_start..position]);
                if closing {
                    self.close_spoiler(&rest[..len]);
                } else {
                    self.open_spoiler(&rest[..len]);
                }
                len
            } else if let Some((index, len)) = parse_marker(rest)
                && let Some((source, html)) = self.take_art(index)
            {
                self.push_plain(&text[plain_start..position]);
                if self.in_link {
                    self.push_plain(source);
                } else {
                    self.push_sjis(html);
                }
                len
            } else if !self.in_link
                && is_entity_boundary(text[..position].chars().next_back())
                && let Some((len, dest_url)) = entity_link(rest)
            {
                self.push_plain(&text[plain_start..position]);
                self.push_link(dest_url, &rest[..len]);
                len
            } else {
                0
            };

            if token_len > 0 {
                position += token_len;
                plain_start = position;
            } else {
                position += next_char.len_utf8();
            }
        }
        self.push_plain(&text[plain_start..]);
    }

    fn open_spoiler(&mut self, token: &str) {
        self.open_spoilers.push(OpenSpoiler {
            index: self.events.len(),
            depth: self.depth,
        });
        self.push_plain(token);
    }

    fn close_spoiler(&mut self, token: &str) {
        match self.open_spoilers.pop_if(|open| open.depth == self.depth) {
            Some(open) if self.events.len() > open.index + 1 => {
                self.events[open.index] = Event::InlineHtml(CowStr::Borrowed(r#"<span class="spoiler">"#));
                self.events.push(Event::InlineHtml(CowStr::Borrowed("</span>")));
            }
            _ => self.push_plain(token),
        }
    }

    fn push_sjis(&mut self, html: String) {
        // Art is displayed as a block, so line breaks next to it would only add blank lines
        while matches!(self.events.last(), Some(Event::HardBreak)) {
            self.events.pop();
        }
        self.events.push(Event::Html(html.into()));
    }

    fn take_art(&mut self, index: usize) -> Option<(&'a str, String)> {
        let art = self.sjis_art.get_mut(index)?;
        art.html.take().map(|html| (art.source, html))
    }

    fn unmark(&mut self, text: CowStr<'a>) -> CowStr<'a> {
        if self.sjis_art.is_empty() || !text.contains(MARKER_START) {
            return text;
        }

        let mut source = String::with_capacity(text.len());
        let mut copied = 0;
        for (index, _) in text.match_indices(MARKER_START) {
            if let Some((art_index, marker_len)) = parse_marker(&text[index..])
                && let Some((art_source, _)) = self.take_art(art_index)
            {
                source.push_str(&text[copied..index]);
                source.push_str(art_source);
                copied = index + marker_len;
            }
        }
        source.push_str(&text[copied..]);
        source.into()
    }

    fn push_plain(&mut self, text: &str) {
        if !text.is_empty() {
            self.events.push(Event::Text(text.to_owned().into()));
        }
    }

    fn push_link(&mut self, dest_url: String, text: &str) {
        self.events.push(Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: dest_url.into(),
            title: CowStr::Borrowed(""),
            id: CowStr::Borrowed(""),
        }));
        self.push_plain(text);
        self.events.push(Event::End(TagEnd::Link));
    }
}

fn is_entity_boundary(previous: Option<char>) -> bool {
    previous.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '<' | '>'))
}

fn entity_link(text: &str) -> Option<(usize, String)> {
    let sigil = *text
        .as_bytes()
        .first()
        .filter(|sigil| matches!(sigil, b'@' | b'+' | b'#'))?;
    let name_len = text[1..]
        .bytes()
        .take_while(|&byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        .count();
    let name = &text[1..=name_len];
    let dest_url = match sigil {
        _ if name.is_empty() => return None,
        b'@' if name.bytes().all(|byte| byte.is_ascii_digit()) => format!("post/{name}"),
        b'+' => format!("user/{name}"),
        b'#' => web::tag_search_url(name),
        _ => return None,
    };
    Some((1 + name_len, dest_url))
}

fn bare_url(text: &str) -> Option<&str> {
    let prefix_len = URL_PREFIXES
        .iter()
        .find(|prefix| strip_prefix_ignore_ascii_case(text, prefix).is_some())?
        .len();
    let end = text
        .char_indices()
        .find(|&(index, c)| c.is_whitespace() || c == '<' || c == MARKER_START || starts_with_tag(&text[index..]))
        .map_or(text.len(), |(index, _)| index);
    let url = trim_url(&text[..end]);
    (url.len() > prefix_len).then_some(url)
}

fn trim_url(mut url: &str) -> &str {
    let open_parens = url.matches('(').count();
    let mut close_parens = url.matches(')').count();
    loop {
        url = url.trim_end_matches(['?', '!', '.', ',', ':', '*', '_', '~', '\'', '"']);
        match url.strip_suffix(')') {
            Some(trimmed) if close_parens > open_parens => {
                url = trimmed;
                close_parens -= 1;
            }
            _ => return url,
        }
    }
}

struct Image {
    url: String,
    width: String,
    height: String,
    alt: String,
}

impl Image {
    fn parse(dest_url: &str) -> Self {
        Self::parse_sized(dest_url).unwrap_or_else(|| Self {
            url: dest_url.to_owned(),
            width: String::new(),
            height: String::new(),
            alt: String::new(),
        })
    }

    // `url =WxH`, where either dimension may be omitted
    fn parse_sized(dest_url: &str) -> Option<Self> {
        let (url, size) = dest_url.rsplit_once('=')?;
        let url = url.strip_suffix(char::is_whitespace)?.trim_end();
        let (width, height) = size.split_once('x')?;
        let (width, height) = (width.trim(), height.trim());
        let is_number = |text: &str| text.bytes().all(|byte| byte.is_ascii_digit());
        (!url.is_empty() && is_number(width) && is_number(height)).then(|| Self {
            url: url.to_owned(),
            width: width.to_owned(),
            height: height.to_owned(),
            alt: String::new(),
        })
    }

    fn to_html(&self) -> String {
        let mut html =
            format!(r#"<img src="{}" alt="{}""#, ammonia::clean_text(&self.url), ammonia::clean_text(&self.alt));
        if !self.width.is_empty() {
            write!(html, r#" width="{}""#, self.width).expect("Write to String is infallible");
        }
        if !self.height.is_empty() {
            write!(html, r#" height="{}""#, self.height).expect("Write to String is infallible");
        }
        html.push('>');
        html
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_input() {
        assert_eq!(render(""), "");
    }

    #[test]
    fn line_breaks() {
        assert_eq!(render("a\nb"), "<p>a<br>\nb</p>\n");
        assert_eq!(render("a\n\nb"), "<p>a</p>\n<p>b</p>\n");
    }

    #[test]
    fn standard_markdown() {
        assert_eq!(render("# Header"), "<h1>Header</h1>\n");
        assert_eq!(render("*a* **b**"), "<p><em>a</em> <strong>b</strong></p>\n");
        assert_eq!(render("~~a~~ ~b~"), "<p><del>a</del> <del>b</del></p>\n");
        assert_eq!(render(r"\~a\~"), "<p>~a~</p>\n");
        assert_eq!(render(r#""a" -- b..."#), "<p>“a” – b…</p>\n");
        assert_eq!(
            render("|a|b|\n|-|-|\n|1|2|"),
            "<table><thead><tr><th>a</th><th>b</th></tr></thead><tbody>\n<tr><td>1</td><td>2</td></tr>\n</tbody></table>\n"
        );
    }

    #[test]
    fn entity_links() {
        assert_eq!(render("@726"), format!("<p>{}</p>\n", link("post/726", "@726")));
        assert_eq!(render("+Pirate"), format!("<p>{}</p>\n", link("user/Pirate", "+Pirate")));
        assert_eq!(
            render("#Dragon_Ball"),
            format!("<p>{}</p>\n", link("posts?search-text=Dragon%5FBall", "#Dragon_Ball"))
        );

        // Entities can follow whitespace, brackets, or an opening paren
        assert_eq!(render("see @1"), format!("<p>see {}</p>\n", link("post/1", "@1")));
        assert_eq!(render("a\n#b"), format!("<p>a<br>\n{}</p>\n", link("posts?search-text=b", "#b")));
        assert_eq!(render("(@1)"), format!("<p>({})</p>\n", link("post/1", "@1")));
        assert_eq!(render("x (+u)"), format!("<p>x ({})</p>\n", link("user/u", "+u")));

        // But not other characters
        assert_eq!(render("a@1"), "<p>a@1</p>\n");
        assert_eq!(render("me@example.com"), "<p>me@example.com</p>\n");
        assert_eq!(render("@abc"), "<p>@abc</p>\n");
    }

    #[test]
    fn entities_dont_break_existing_links() {
        assert_eq!(render("[text](#anchor)"), format!("<p>{}</p>\n", link("#anchor", "text")));
        assert_eq!(
            render("https://example.com/#section"),
            format!("<p>{}</p>\n", link("https://example.com/#section", "https://example.com/#section"))
        );
    }

    #[test]
    fn spoilers() {
        assert_eq!(render("[spoiler]a[/spoiler]"), "<p><span class=\"spoiler\">a</span></p>\n");
        assert_eq!(render("[SPOILER]a[/Spoiler]"), "<p><span class=\"spoiler\">a</span></p>\n");
        assert_eq!(render("[spoiler]*a*[/spoiler]"), "<p><span class=\"spoiler\"><em>a</em></span></p>\n");
        assert_eq!(render("[spoiler]a\nb[/spoiler]"), "<p><span class=\"spoiler\">a<br>\nb</span></p>\n");
    }

    #[test]
    fn unsupported_tags_stay_literal() {
        assert_eq!(render("[small]a[/small]"), "<p>[small]a[/small]</p>\n");
        assert_eq!(render("[search]a[/search]"), "<p>[search]a[/search]</p>\n");
    }

    #[test]
    fn malformed_tags() {
        assert_eq!(render("[spoiler][/spoiler]"), "<p>[spoiler][/spoiler]</p>\n");
        assert_eq!(render("[spoiler]a"), "<p>[spoiler]a</p>\n");
        assert_eq!(render("a[/spoiler]"), "<p>a[/spoiler]</p>\n");
        assert_eq!(render("[spoiler]a[spoiler]b[/spoiler]"), "<p>[spoiler]a<span class=\"spoiler\">b</span></p>\n");
        assert_eq!(
            render("[spoiler]a[/spoiler] [spoiler]b[/spoiler]"),
            "<p><span class=\"spoiler\">a</span> <span class=\"spoiler\">b</span></p>\n"
        );
    }

    #[test]
    fn sjis() {
        assert_eq!(render("[sjis] *a*  _b_ [/sjis]"), "<span class=\"sjis\"> *a*  _b_ </span>");
        assert_eq!(render("[sjis]<b>@1</b>[/sjis]"), "<span class=\"sjis\">&lt;b&gt;@1&lt;/b&gt;</span>");
        assert_eq!(render("[sjis]a\n\nb[/sjis]"), "<span class=\"sjis\">a\n\nb</span>");
        assert_eq!(render("[sjis]\na\n[/sjis]"), "<span class=\"sjis\">a</span>");
        assert_eq!(render("[sjis]  \n a \n  [/sjis]"), "<span class=\"sjis\"> a </span>");
        assert_eq!(render("[sjis]\r\na\r\n[/sjis]"), "<span class=\"sjis\">a</span>");
        assert_eq!(render("[sjis] a\n[/sjis]"), "<span class=\"sjis\"> a</span>");
        assert_eq!(render("[sjis]\n\na[/sjis]"), "<span class=\"sjis\">\na</span>");
    }

    #[test]
    fn icon() {
        assert_eq!(
            render("[icon]https://youtube.com[/icon]"),
            format!("<p>{}</p>\n", link("https://youtube.com", "https://youtube.com"))
        );
        assert_eq!(render("[icon]www.a.com[/icon]"), format!("<p>{}</p>\n", link("http://www.a.com", "www.a.com")));
        assert_eq!(render("[icon]javascript:alert(1)[/icon]"), "<p>[icon]javascript:alert(1)[/icon]</p>\n");
        assert_eq!(render("[icon]a.com[/icon]"), "<p>[icon]a.com[/icon]</p>\n");
        assert_eq!(
            render("[icon]https://a.com b[/icon]"),
            format!("<p>[icon]{} b[/icon]</p>\n", link("https://a.com", "https://a.com"))
        );
        assert_eq!(
            render("[icon]https://a.com/.[/icon]"),
            format!("<p>{}</p>\n", link("https://a.com/.", "https://a.com/."))
        );
        assert_eq!(
            render("[icon][ICON]https://a.com[/Icon]"),
            format!("<p>[icon]{}</p>\n", link("https://a.com", "https://a.com"))
        );
        assert_eq!(render("[icon]https://[/icon]"), "<p>[icon]https://[/icon]</p>\n");
        assert_eq!(
            render("[icon]https://a.com/a--b[/icon]"),
            format!("<p>{}</p>\n", link("https://a.com/a--b", "https://a.com/a--b"))
        );
    }

    #[test]
    fn tags_are_ignored_in_code() {
        assert_eq!(
            render("```\nsee @1 and #tag [spoiler]a[/spoiler]\n```"),
            "<pre><code>see @1 and #tag [spoiler]a[/spoiler]\n</code></pre>\n"
        );
        assert_eq!(render("`see +user`"), "<p><code>see +user</code></p>\n");
        assert_eq!(render("`[spoiler]a[/spoiler]`"), "<p><code>[spoiler]a[/spoiler]</code></p>\n");
        assert_eq!(render("`[icon]https://a.com[/icon]`"), "<p><code>[icon]https://a.com[/icon]</code></p>\n");
    }

    #[test]
    fn spoilers_dont_cross_containers() {
        assert_eq!(render("[spoiler]a\n\nb[/spoiler]"), "<p>[spoiler]a</p>\n<p>b[/spoiler]</p>\n");
        assert_eq!(render("*a [spoiler]b* c[/spoiler]"), "<p><em>a [spoiler]b</em> c[/spoiler]</p>\n");
    }

    #[test]
    fn tags_in_link_text() {
        assert_eq!(
            render("[[spoiler]a[/spoiler]](https://a.com)"),
            format!("<p>{}</p>\n", link("https://a.com", "<span class=\"spoiler\">a</span>"))
        );
        assert_eq!(
            render("[[icon]https://b.com[/icon]](https://a.com)"),
            format!("<p>{}</p>\n", link("https://a.com", "[icon]https://b.com[/icon]"))
        );
        assert_eq!(
            render("[see @1 https://b.com](https://a.com)"),
            format!("<p>{}</p>\n", link("https://a.com", "see @1 https://b.com"))
        );
    }

    #[test]
    fn bare_urls_stop_at_tags() {
        assert_eq!(
            render("[spoiler]https://a.com[/spoiler]"),
            format!("<p><span class=\"spoiler\">{}</span></p>\n", link("https://a.com", "https://a.com"))
        );
    }

    #[test]
    fn entities_next_to_multibyte_characters() {
        assert_eq!(render("é@1 “#a”"), "<p>é@1 “#a”</p>\n");
        assert_eq!(render("→ @1"), format!("<p>→ {}</p>\n", link("post/1", "@1")));
    }

    #[test]
    fn sjis_in_paragraphs() {
        assert_eq!(render("a\n[sjis]x[/sjis]\nb"), "<p>a<span class=\"sjis\">x</span>b</p>\n");
        assert_eq!(render("[sjis]a[sjis]b[/sjis]"), "<p>[sjis]a<span class=\"sjis\">b</span></p>\n");
        assert_eq!(
            render("[spoiler][sjis]x[/sjis][/spoiler]"),
            "<p><span class=\"spoiler\"><span class=\"sjis\">x</span></span></p>\n"
        );
        assert_eq!(render("*[sjis]x[/sjis]*"), "<p><em><span class=\"sjis\">x</span></em></p>\n");
        assert_eq!(render("[SJIS][/sjis]"), "<p>[SJIS][/sjis]</p>\n");
    }

    #[test]
    fn sjis_is_literal_in_code_and_links() {
        assert_eq!(render("`[sjis]x[/sjis]`"), "<p><code>[sjis]x[/sjis]</code></p>\n");
        assert_eq!(render("```\n[sjis]x[/sjis]\n```"), "<pre><code>[sjis]x[/sjis]\n</code></pre>\n");
        assert_eq!(
            render("[a](https://a.com/[sjis]x[/sjis])"),
            format!("<p>{}</p>\n", link("https://a.com/%5Bsjis%5Dx%5B/sjis%5D", "a"))
        );
        assert_eq!(render("<div>[sjis]x[/sjis]</div>"), "<div>[sjis]x[/sjis]</div>");
        assert_eq!(
            render("[see [sjis]x[/sjis]](https://a.com)"),
            format!("<p>{}</p>\n", link("https://a.com", "see [sjis]x[/sjis]"))
        );
        assert_eq!(render("`a [sjis]`b`[/sjis]`"), "<p><code>a [sjis]`b`[/sjis]</code></p>\n");
    }

    #[test]
    fn sjis_keeps_markdown_structure() {
        assert_eq!(
            render("- a\n  [sjis]x[/sjis]\n- b"),
            "<ul>\n<li>a<span class=\"sjis\">x</span></li>\n<li>b</li>\n</ul>\n"
        );
        assert_eq!(render("- a\n  [sjis]x[/sjis]\n  b"), "<ul>\n<li>a<span class=\"sjis\">x</span>b</li>\n</ul>\n");
        assert_eq!(
            render("[1]: https://a.com\n\n[sjis]x[/sjis] [link][1]"),
            format!("<p><span class=\"sjis\">x</span> {}</p>\n", link("https://a.com", "link"))
        );
    }

    #[test]
    fn sjis_art_is_untouched() {
        assert_eq!(
            render("[sjis]    indented\n\n`tick` *a* @1 https://a.com <b>\n[/sjis]"),
            "<span class=\"sjis\">    indented\n\n`tick` *a* @1 https://a.com &lt;b&gt;</span>"
        );
    }

    #[test]
    fn forged_sjis_markers() {
        assert_eq!(render("\u{E000}0\u{E001}"), "<p>\u{E000}0\u{E001}</p>\n");
        assert_eq!(
            render("[sjis]x[/sjis] \u{E000}0\u{E001}"),
            "<p><span class=\"sjis\">x</span> \u{FFFD}0\u{FFFD}</p>\n"
        );
        assert_eq!(render("[sjis]\u{E000}0\u{E001}[/sjis]"), "<span class=\"sjis\">\u{E000}0\u{E001}</span>");
    }

    #[test]
    fn art_cant_be_multiplied() {
        let art = "x".repeat(10_000);
        let typed_markers = format!("[sjis]{art}[/sjis]\n\n{}", "\u{E000}0\u{E001} ".repeat(1_000));
        let markers_in_code = format!("[sjis]{art}[/sjis]\n\n`{}`", "\u{E000}0\u{E001}".repeat(1_000));
        let reference_links = format!("[r]: https://a.com/[sjis]{art}[/sjis]\n\n{}", "[a][r] ".repeat(1_000));
        for input in [typed_markers, markers_in_code, reference_links] {
            assert_eq!(render(&input).matches(&art).count(), 1);
        }
    }

    #[test]
    fn external_images_become_links() {
        assert_eq!(
            render("![cat](https://a.com/cat.png)"),
            format!("<p>{}</p>\n", link("https://a.com/cat.png", "cat"))
        );
        assert_eq!(
            render("![](https://a.com/cat.png)"),
            format!("<p>{}</p>\n", link("https://a.com/cat.png", "https://a.com/cat.png"))
        );
        assert_eq!(
            render("![*cat*](<https://a.com/cat.png =100x50>)"),
            format!("<p>{}</p>\n", link("https://a.com/cat.png", "cat"))
        );
        assert_eq!(render("![cat](//a.com/cat.png)"), format!("<p>{}</p>\n", link("//a.com/cat.png", "cat")));
        assert_eq!(render("![cat](data/cat.png)"), "<p><img src=\"data/cat.png\" alt=\"cat\"></p>\n");
    }

    #[test]
    fn external_images_in_links() {
        assert_eq!(
            render("[![badge](https://a.com/b.png)](https://a.com)"),
            format!("<p>{}</p>\n", link("https://a.com", "badge"))
        );
        assert_eq!(
            render("[![](https://a.com/b.png)](https://a.com)"),
            format!("<p>{}</p>\n", link("https://a.com", "https://a.com/b.png"))
        );
    }

    #[test]
    fn nested_images() {
        assert_eq!(
            render("![![a](b.png) x](<c.png =1x1>) after [spoiler]s[/spoiler]"),
            "<p><img src=\"c.png\" alt=\"a x\" width=\"1\" height=\"1\"> after <span class=\"spoiler\">s</span></p>\n"
        );
        assert_eq!(
            render("![![a](https://a.com/b.png) x](c.png) after [spoiler]s[/spoiler]"),
            "<p><img src=\"c.png\" alt=\"a x\"> after <span class=\"spoiler\">s</span></p>\n"
        );
    }

    #[test]
    fn external_resources_are_stripped() {
        assert_eq!(render("<img src=\"https://a.com/t.png\">"), "<img>");
        assert_eq!(render("<img src=\"/\\a.com/t.png\">"), "<img>");
        assert_eq!(render("<img src=\"data/t.png\">"), "<img src=\"data/t.png\">");
        for style in [
            "background: url(https://a.com/t.png)",
            "background: URL(https://a.com/t.png)",
            "background-image: image-set('https://a.com/t.png' 1x)",
            "background: \\75 rl(https://a.com/t.png)",
        ] {
            assert_eq!(render(&format!("<span style=\"{style}\">a</span>")), "<p><span>a</span></p>\n");
        }
    }

    #[test]
    fn sized_images() {
        assert_eq!(
            render("![alt](<a.png =100x50>)"),
            "<p><img src=\"a.png\" alt=\"alt\" width=\"100\" height=\"50\"></p>\n"
        );
        assert_eq!(render("![](<a.png =100x>)"), "<p><img src=\"a.png\" alt=\"\" width=\"100\"></p>\n");
        assert_eq!(render("![](<a.png =x50>)"), "<p><img src=\"a.png\" alt=\"\" height=\"50\"></p>\n");
        assert_eq!(
            render("![*a* `b`](<a.png =1x1>)"),
            "<p><img src=\"a.png\" alt=\"a b\" width=\"1\" height=\"1\"></p>\n"
        );
        assert_eq!(render("![alt](a.png)"), "<p><img src=\"a.png\" alt=\"alt\"></p>\n");
    }

    #[test]
    fn autolinks() {
        assert_eq!(
            render("go to https://example.com/a?b=c."),
            format!("<p>go to {}.</p>\n", link("https://example.com/a?b=c", "https://example.com/a?b=c"))
        );
        assert_eq!(
            render("www.example.com"),
            format!("<p>{}</p>\n", link("http://www.example.com", "www.example.com"))
        );
        assert_eq!(render("(https://a.com/b)"), format!("<p>({})</p>\n", link("https://a.com/b", "https://a.com/b")));
        assert_eq!(
            render("https://a.com/b_(c)"),
            format!("<p>{}</p>\n", link("https://a.com/b_(c)", "https://a.com/b_(c)"))
        );
        assert_eq!(render("ftp://a.com"), "<p>ftp://a.com</p>\n");
    }

    #[test]
    fn bare_urls_are_literal() {
        for url in [
            "https://a.com/a--b",
            "https://a.com/it's",
            "https://a.com/a...b",
            "https://a.com/_x_/y",
            "https://a.com/*a*b",
            "https://a.com/a&amp;b",
        ] {
            assert_eq!(
                render(url),
                format!("<p>{}</p>\n", link(&url.replace('&', "&amp;"), &url.replace('&', "&amp;")))
            );
        }
        assert_eq!(render("www.a.com/a--b"), format!("<p>{}</p>\n", link("http://www.a.com/a--b", "www.a.com/a--b")));
    }

    #[test]
    fn bare_urls_inside_markdown() {
        assert_eq!(
            render("*see https://a.com*"),
            format!("<p><em>see {}</em></p>\n", link("https://a.com", "https://a.com"))
        );
        assert_eq!(
            render("|https://a.com/a|b|\n|-|-|"),
            format!(
                "<table><thead><tr><th>{}</th><th>b</th></tr></thead><tbody>\n</tbody></table>\n",
                link("https://a.com/a", "https://a.com/a")
            )
        );
    }

    #[test]
    fn bare_urls_in_markup_are_untouched() {
        assert_eq!(
            render("[a]: https://a.com/a--b\n\n[x][a]"),
            format!("<p>{}</p>\n", link("https://a.com/a--b", "x"))
        );
        assert_eq!(render("<a href=\"https://a.com\">x</a>"), format!("<p>{}</p>\n", link("https://a.com", "x")));
        assert_eq!(render("<div>https://a.com</div>"), "<div>https://a.com</div>");
        assert_eq!(
            render("<https://a.com/a--b>"),
            format!("<p>{}</p>\n", link("https://a.com/a--b", "https://a.com/a--b"))
        );
    }

    #[test]
    fn url_prefixes_ignore_case() {
        assert_eq!(render("HTTPS://A.com/X"), format!("<p>{}</p>\n", link("HTTPS://A.com/X", "HTTPS://A.com/X")));
        assert_eq!(render("WWW.a.com"), format!("<p>{}</p>\n", link("http://WWW.a.com", "WWW.a.com")));
        assert_eq!(render("[ICON]Http://a.com[/icon]"), format!("<p>{}</p>\n", link("Http://a.com", "Http://a.com")));
    }

    #[test]
    fn no_autolinks_inside_links_or_code() {
        assert_eq!(
            render("[https://a.com](https://b.com)"),
            format!("<p>{}</p>\n", link("https://b.com", "https://a.com"))
        );
        assert_eq!(render("`https://a.com`"), "<p><code>https://a.com</code></p>\n");
        assert_eq!(render("```\nhttps://a.com\n```"), "<pre><code>https://a.com\n</code></pre>\n");
        assert_eq!(render("    https://a.com"), "<pre><code>https://a.com</code></pre>\n");
    }

    #[test]
    fn sanitization() {
        assert_eq!(render("<script>alert(1)</script>"), "");
        assert_eq!(render("<b onclick=\"alert(1)\">a</b>"), "<p><b>a</b></p>\n");
        assert_eq!(render("<a href=\"javascript:alert(1)\">a</a>"), "<p><a rel=\"noopener noreferrer\">a</a></p>\n");
        assert_eq!(render("[a](javascript:alert(1))"), "<p><a rel=\"noopener noreferrer\">a</a></p>\n");
        assert_eq!(render("<span hx-get=\"/x\" data-a=\"b\" id=\"c\">a</span>"), "<p><span>a</span></p>\n");
        assert_eq!(render("<span style=\"color: red\">a</span>"), "<p><span style=\"color: red\">a</span></p>\n");
    }

    #[test]
    fn only_markup_classes_are_allowed() {
        assert_eq!(render("<span class=\"spoiler x\">a</span>"), "<p><span class=\"spoiler\">a</span></p>\n");
        assert_eq!(render("<span class=\"sjis comment\">a</span>"), "<p><span class=\"sjis\">a</span></p>\n");
        assert_eq!(render("<div class=\"sjis\">a</div>"), "<div>a</div>");
    }

    #[test]
    fn structural_tags_are_unwrapped() {
        assert_eq!(render("<header><nav>a</nav></header><article><aside>b</aside><footer>c</footer></article>"), "abc");
    }

    fn link(href: &str, text: &str) -> String {
        format!(r#"<a href="{href}" rel="noopener noreferrer">{text}</a>"#)
    }
}
