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

    let mut html = String::new();
    html::push_html(&mut html, Renderer::new(sjis_art).run(parser).into_iter());
    SANITIZER.clean(&html).to_string()
}

const SPOILER_OPEN: &str = "[spoiler]";
const SPOILER_CLOSE: &str = "[/spoiler]";
const ICON_OPEN: &str = "[icon]";
const ICON_CLOSE: &str = "[/icon]";
const SJIS_OPEN: &str = "[sjis]";
const SJIS_CLOSE: &str = "[/sjis]";
const MARKER_START: char = '\u{E000}';
const MARKER_END: char = '\u{E001}';

const URL_PREFIXES: [&str; 3] = ["https://", "http://", "www."];
const ENTITY_SIGILS: [char; 3] = ['@', '+', '#'];

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

struct SjisMatch<'a, 'b> {
    art: SjisArt<'a>,
    before: &'a str,
    after: &'a str,
    after_lowercase: &'b str,
}

struct SjisMarker {
    index: usize,
    len: usize,
}

struct EntityLink {
    len: usize,
    text: String,
    dest_url: String,
}

struct TextStructure {
    verbatim_ranges: Vec<Range<usize>>,
    cell_ends: Vec<usize>,
}

struct OpenSpoiler {
    index: usize,
    depth: usize,
}

enum SpoilerTag {
    Open,
    Close,
}

impl SpoilerTag {
    fn len(&self) -> usize {
        match self {
            Self::Open => SPOILER_OPEN,
            Self::Close => SPOILER_CLOSE,
        }
        .len()
    }
}

struct Image {
    url: String,
    width: Option<u16>,
    height: Option<u16>,
    alt: String,
}

impl Image {
    fn parse(dest_url: &str) -> Self {
        Self::parse_sized(dest_url).unwrap_or_else(|| Self {
            url: dest_url.to_owned(),
            width: None,
            height: None,
            alt: String::new(),
        })
    }

    // `url =WxH`, where either dimension may be omitted
    fn parse_sized(dest_url: &str) -> Option<Self> {
        let (url, size) = dest_url.rsplit_once('=')?;
        let url = url.strip_suffix(char::is_whitespace)?.trim_end();
        if url.is_empty() {
            return None;
        }

        let dimension = |text: &str| match text.trim() {
            "" => Some(None),
            text => text.parse().ok().map(Some),
        };
        let (width, height) = size.split_once('x')?;
        let (width, height) = (dimension(width)?, dimension(height)?);
        Some(Self {
            url: url.to_owned(),
            width,
            height,
            alt: String::new(),
        })
    }

    fn to_html(&self) -> String {
        let mut html =
            format!(r#"<img src="{}" alt="{}""#, ammonia::clean_text(&self.url), ammonia::clean_text(&self.alt));
        if let Some(width) = self.width {
            write!(html, r#" width="{width}""#).expect("Write to String is infallible");
        }
        if let Some(height) = self.height {
            write!(html, r#" height="{height}""#).expect("Write to String is infallible");
        }
        html.push('>');
        html
    }
}

struct Renderer<'a> {
    events: Vec<Event<'a>>,
    open_spoilers: Vec<OpenSpoiler>,
    sjis_art: Vec<SjisArt<'a>>,
    depth: usize,
    in_link: bool,
    in_code_block: bool,
}

impl<'a> Renderer<'a> {
    fn new(sjis_art: Vec<SjisArt<'a>>) -> Self {
        Self {
            events: Vec::new(),
            open_spoilers: Vec::new(),
            sjis_art,
            depth: 0,
            in_link: false,
            in_code_block: false,
        }
    }

    fn run(mut self, parser: impl Iterator<Item = Event<'a>>) -> Vec<Event<'a>> {
        let mut images_in_alt = 0;
        let mut image: Option<Image> = None;
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
                    title, // We don't use title, so no need to unmark it
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
            let token_len = if let Some(tag) = spoiler_tag(rest) {
                self.push_plain(&text[plain_start..position]);
                match tag {
                    SpoilerTag::Open => self.open_spoiler(&rest[..tag.len()]),
                    SpoilerTag::Close => self.close_spoiler(&rest[..tag.len()]),
                }
                tag.len()
            } else if let Some(marker) = parse_marker(rest)
                && let Some((source, html)) = self.take_art(marker.index)
            {
                self.push_plain(&text[plain_start..position]);
                if self.in_link {
                    self.push_plain(source);
                } else {
                    self.push_sjis(html);
                }
                marker.len
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

    fn push_plain(&mut self, text: &str) {
        if !text.is_empty() {
            self.events.push(Event::Text(text.to_owned().into()));
        }
    }

    fn push_link(&mut self, dest_url: String, text: &str) {
        self.events.push(Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url: CowStr::from(dest_url),
            title: CowStr::Borrowed(""),
            id: CowStr::Borrowed(""),
        }));
        self.push_plain(text);
        self.events.push(Event::End(TagEnd::Link));
    }

    fn push_sjis(&mut self, html: String) {
        // Art is displayed as a block, so line breaks next to it would only add blank lines
        while self.events.last() == Some(&Event::HardBreak) {
            self.events.pop();
        }
        self.events.push(Event::Html(CowStr::from(html)));
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

    fn unmark(&mut self, text: CowStr<'a>) -> CowStr<'a> {
        if self.sjis_art.is_empty() || !text.contains(MARKER_START) {
            return text;
        }

        let mut copied = 0;
        let mut source = String::with_capacity(text.len());
        for (i, _) in text.match_indices(MARKER_START) {
            if let Some(marker) = parse_marker(&text[i..])
                && let Some((art_source, _)) = self.take_art(marker.index)
            {
                source.push_str(&text[copied..i]);
                source.push_str(art_source);
                copied = i + marker.len;
            }
        }
        source.push_str(&text[copied..]);
        CowStr::from(source)
    }

    fn take_art(&mut self, index: usize) -> Option<(&'a str, String)> {
        let art = self.sjis_art.get_mut(index)?;
        art.html.take().map(|html| (art.source, html))
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

    let mut sjis_art = Vec::new();
    let mut marked = String::with_capacity(text.len());
    let mut rest = text;
    let mut rest_lowercase = lowercase.as_str();
    while let Some(sjis) = find_sjis(rest, rest_lowercase) {
        push_without_markers(&mut marked, sjis.before);
        marked.push(MARKER_START);
        marked.push_str(&sjis_art.len().to_string());
        marked.push(MARKER_END);

        sjis_art.push(sjis.art);
        (rest, rest_lowercase) = (sjis.after, sjis.after_lowercase);
    }
    push_without_markers(&mut marked, rest);
    (Cow::Owned(marked), sjis_art)
}

fn find_sjis<'a, 'b>(text: &'a str, lowercase: &'b str) -> Option<SjisMatch<'a, 'b>> {
    let mut position = 0;
    loop {
        let start = position + lowercase.get(position..)?.find(SJIS_OPEN)?;
        let content_start = start + SJIS_OPEN.len();
        let content_end = content_start + lowercase.get(content_start..)?.find(SJIS_CLOSE)?;
        let end = content_end + SJIS_CLOSE.len();

        let lowercase_inner = lowercase.get(content_start..content_end)?;
        match lowercase_inner.rfind(SJIS_OPEN) {
            Some(inner) => position = content_start + inner,
            None if lowercase_inner.is_empty() => position = start + 1, // Skip empty SJIS blocks
            None => {
                // Lines holding only the tags aren't part of the art, so they don't add blank lines
                let mut inner = text.get(content_start..content_end)?;
                if let Some((first_line, rest)) = inner.split_once('\n')
                    && first_line.trim().is_empty()
                {
                    inner = rest;
                }
                if let Some((rest, last_line)) = inner.rsplit_once('\n')
                    && last_line.trim().is_empty()
                {
                    inner = rest.trim_suffix('\r');
                }

                let (before, rest) = text.split_at_checked(start)?;
                let (source, after) = rest.split_at_checked(end - start)?;
                let after_lowercase = lowercase.get(end..)?;

                let inner = ammonia::clean_text(inner);
                let html = Some(format!(r#"<span class="sjis">{inner}</span>"#));
                let art = SjisArt { source, html };
                return Some(SjisMatch {
                    art,
                    before,
                    after,
                    after_lowercase,
                });
            }
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

/// Returns the art index and the marker's length
fn parse_marker(text: &str) -> Option<SjisMarker> {
    const MAX_MARKER_DIGITS: usize = usize::MAX.ilog10() as usize + 1;

    // Get index of SJIS art
    let rest = text.strip_prefix(MARKER_START)?;
    let digit_count = rest
        .bytes()
        .take(MAX_MARKER_DIGITS)
        .take_while(u8::is_ascii_digit)
        .count();
    let (digits, rest) = rest.split_at(digit_count);

    let index = digits.parse().ok()?;
    let len = MARKER_START.len_utf8() + digit_count + MARKER_END.len_utf8();
    rest.starts_with(MARKER_END).then_some(SjisMarker { index, len })
}

/// Rewrites `text` so that bare URLs, icon blocks, and `@post`, `+user`, and `#tag` entities become escaped Markdown
/// links before parsing, so emphasis and smart punctuation can't alter them. Entities are found here rather than in
/// the parsed text because Markdown would consume the backslashes that escape punctuation in their names.
fn rewrite_source(text: &str) -> Cow<'_, str> {
    if !text.contains(ENTITY_SIGILS)
        && !URL_PREFIXES
            .iter()
            .any(|prefix| contains_ignore_ascii_case(text, prefix))
    {
        return Cow::Borrowed(text);
    }

    let TextStructure {
        verbatim_ranges,
        cell_ends,
    } = scan_structure(text);
    let mut verbatim_ranges = verbatim_ranges.iter().peekable();
    let mut cell_ends = cell_ends.into_iter().peekable();
    let mut linked = String::with_capacity(text.len());
    let mut copied = 0;
    let mut position = 0;
    while position < text.len() {
        while verbatim_ranges.next_if(|range| position >= range.end).is_some() {}
        while cell_ends.next_if(|&end| position >= end).is_some() {}
        let cell_end = cell_ends.peek().copied().unwrap_or(text.len());
        let limit = match verbatim_ranges.peek() {
            Some(range) if position >= range.start => {
                position = range.end;
                continue;
            }
            Some(range) => std::cmp::min(range.start, cell_end),
            None => cell_end,
        };

        let rest = &text[position..limit];
        if let Some(url) = icon(rest) {
            linked.push_str(&text[copied..position]);
            push_url_link(&mut linked, url);
            position += ICON_OPEN.len() + url.len() + ICON_CLOSE.len();
            copied = position;
        } else if let Some(url) = bare_url(rest) {
            linked.push_str(&text[copied..position]);
            push_url_link(&mut linked, url);
            position += url.len();
            copied = position;
        } else if let previous = text[..position].chars().next_back()
            && is_entity_boundary(previous)
            && let Some(entity) = entity_link(rest, previous == Some('_'))
        {
            linked.push_str(&text[copied..position]);
            push_markdown_link(&mut linked, &entity.text, &entity.dest_url);
            position += entity.len;
            copied = position;
        } else {
            position += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    linked.push_str(&text[copied..]);
    Cow::Owned(linked)
}

/// Finds source ranges that aren't prose (code, links, raw HTML, and link reference definitions)
/// and where table cells end, since a cell's closing `|` can directly follow a URL and would
/// otherwise be read as part of it.
fn scan_structure(text: &str) -> TextStructure {
    let parser = Parser::new_ext(text, OPTIONS);

    let mut cell_ends = Vec::new();
    let mut verbatim_ranges: Vec<_> = parser
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
            | Event::InlineHtml(_) => verbatim_ranges.push(range),
            _ => (),
        }
    }
    verbatim_ranges.sort_unstable_by_key(|range| range.start);
    TextStructure {
        verbatim_ranges,
        cell_ends,
    }
}

fn push_url_link(markdown: &mut String, url: &str) {
    markdown.push('[');
    push_escaped_markdown(markdown, url);
    markdown.push_str("](<");
    if strip_prefix_ignore_ascii_case(url, "www.").is_some() {
        push_escaped_markdown(markdown, "http://");
    }
    push_escaped_markdown(markdown, url);
    markdown.push_str(">)");
}

fn push_markdown_link(markdown: &mut String, text: &str, dest_url: &str) {
    markdown.push('[');
    push_escaped_markdown(markdown, text);
    markdown.push_str("](<");
    push_escaped_markdown(markdown, dest_url);
    markdown.push_str(">)");
}

fn push_escaped_markdown(markdown: &mut String, text: &str) {
    for c in text.chars() {
        if c.is_ascii_punctuation() {
            markdown.push('\\');
        }
        markdown.push(c);
    }
}

/// Returns whether text starts with a opening or closing spoiler tag.
fn spoiler_tag(text: &str) -> Option<SpoilerTag> {
    if strip_prefix_ignore_ascii_case(text, SPOILER_OPEN).is_some() {
        Some(SpoilerTag::Open)
    } else if strip_prefix_ignore_ascii_case(text, SPOILER_CLOSE).is_some() {
        Some(SpoilerTag::Close)
    } else {
        None
    }
}

/// Returns the URL inside a `[icon]url[/icon]` block.
fn icon(text: &str) -> Option<&str> {
    let content = strip_prefix_ignore_ascii_case(text, ICON_OPEN)?;
    let url_end = content
        .find(|c: char| c.is_whitespace() || matches!(c, '[' | '<' | MARKER_START))
        .unwrap_or(content.len());
    let (url, rest) = content.split_at(url_end);
    strip_prefix_ignore_ascii_case(rest, ICON_CLOSE)?;
    URL_PREFIXES
        .iter()
        .any(|prefix| strip_prefix_ignore_ascii_case(url, prefix).is_some())
        .then_some(url)
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
        .and_then(|_| text.get(prefix.len()..))
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
    const TRAILING_URL_PUNCTUATION: [char; 10] = ['?', '!', '.', ',', ':', '*', '_', '~', '\'', '"'];

    let open_parens = url.matches('(').count();
    let mut close_parens = url.matches(')').count();
    loop {
        url = url.trim_end_matches(TRAILING_URL_PUNCTUATION);
        match url.strip_suffix(')') {
            Some(trimmed) if close_parens > open_parens => {
                url = trimmed;
                close_parens -= 1;
            }
            _ => return url,
        }
    }
}

fn is_entity_boundary(previous: Option<char>) -> bool {
    previous.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '<' | '>' | '|' | '*' | '_' | '~'))
}

fn entity_link(text: &str, trim_underscores: bool) -> Option<EntityLink> {
    let sigil = text.chars().next().filter(|&sigil| ENTITY_SIGILS.contains(&sigil))?;
    let rest = &text[sigil.len_utf8()..];

    // Like in search, `\` makes the next character part of the name
    let mut name = String::new();
    let mut name_len = 0;
    let mut untrimmed = (0, 0);
    let mut escaped = false;
    for (index, c) in rest.char_indices() {
        if escaped && !c.is_whitespace() || !escaped && (c.is_alphanumeric() || matches!(c, '_' | '-')) {
            name.push(c);
            name_len = index + c.len_utf8();
            if escaped || c != '_' {
                untrimmed = (name.len(), name_len);
            }
            escaped = false;
        } else if !escaped && c == '\\' {
            escaped = true;
        } else {
            break;
        }
    }

    // After an opening `_`, trailing underscores close the emphasis instead. `\_` keeps one in the name.
    if trim_underscores {
        name.truncate(untrimmed.0);
        name_len = untrimmed.1;
    }
    if name.is_empty() {
        return None;
    }

    let dest_url = match sigil {
        '@' if name.bytes().all(|b| b.is_ascii_digit()) => format!("post/{name}"),
        '+' => web::user_url(&name),
        '#' => web::tag_search_url(&name),
        _ => return None,
    };
    Some(EntityLink {
        len: sigil.len_utf8() + name_len,
        text: format!("{sigil}{name}"),
        dest_url,
    })
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
        assert_eq!(render("@１"), "<p>@１</p>\n");

        // Names can be Unicode, but stop at punctuation
        assert_eq!(render("#café"), format!("<p>{}</p>\n", link("posts?search-text=caf%C3%A9", "#café")));
        assert_eq!(
            render("+ユーザー"),
            format!("<p>{}</p>\n", link("user/%E3%83%A6%E3%83%BC%E3%82%B6%E3%83%BC", "+ユーザー"))
        );
        assert_eq!(render("see #tag."), format!("<p>see {}.</p>\n", link("posts?search-text=tag", "#tag")));
        assert_eq!(render("#a--b"), format!("<p>{}</p>\n", link("posts?search-text=a%2D%2Db", "#a--b")));

        // Punctuation can be escaped like in search
        assert_eq!(render("+user\\.name"), format!("<p>{}</p>\n", link("user/user%2Ename", "+user.name")));
        assert_eq!(render("#a\\:b."), format!("<p>{}.</p>\n", link("posts?search-text=a%5C%3Ab", "#a:b")));
        assert_eq!(render("#a\\\\b"), format!("<p>{}</p>\n", link("posts?search-text=a%5C%5Cb", "#a\\b")));
        assert_eq!(render("#tag\\ x"), format!("<p>{}\\ x</p>\n", link("posts?search-text=tag", "#tag")));
        assert_eq!(render("\\#tag"), "<p>#tag</p>\n");

        // Markup around entities isn't part of their names
        assert_eq!(render("*@1*"), format!("<p><em>{}</em></p>\n", link("post/1", "@1")));
        assert_eq!(render("~~#a~~"), format!("<p><del>{}</del></p>\n", link("posts?search-text=a", "#a")));
        assert_eq!(render("_#a_b_"), format!("<p><em>{}</em></p>\n", link("posts?search-text=a%5Fb", "#a_b")));
        assert_eq!(render("__#a__"), format!("<p><strong>{}</strong></p>\n", link("posts?search-text=a", "#a")));
        assert_eq!(render("_#a\\__"), format!("<p><em>{}</em></p>\n", link("posts?search-text=a%5F", "#a_")));
        assert_eq!(render("#a_"), format!("<p>{}</p>\n", link("posts?search-text=a%5F", "#a_")));
        assert_eq!(
            render("|#a|b|\n|-|-|"),
            format!(
                "<table><thead><tr><th>{}</th><th>b</th></tr></thead><tbody>\n</tbody></table>\n",
                link("posts?search-text=a", "#a")
            )
        );
        assert_eq!(render("`#a` [x](#a)"), format!("<p><code>#a</code> {}</p>\n", link("#a", "x")));
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
    fn nested_sjis() {
        // The innermost block wins and the outer tags stay literal
        assert_eq!(render("[sjis]a[sjis]b[/sjis]c[/sjis]"), "<p>[sjis]a<span class=\"sjis\">b</span>c[/sjis]</p>\n");
        assert_eq!(render("[sjis][sjis]a[/sjis][/sjis]"), "<p>[sjis]<span class=\"sjis\">a</span>[/sjis]</p>\n");
        assert_eq!(
            render("[sjis]a[sjis]b[sjis]c[/sjis]d[/sjis]e[/sjis]"),
            "<p>[sjis]a[sjis]b<span class=\"sjis\">c</span>d[/sjis]e[/sjis]</p>\n"
        );
        assert_eq!(render("[SJIS]a[Sjis]b[/SJIS]c[/sjis]"), "<p>[SJIS]a<span class=\"sjis\">b</span>c[/sjis]</p>\n");
        assert_eq!(
            render("[sjis]a[sjis]b[/sjis][sjis]c[/sjis][/sjis]"),
            "<p>[sjis]a<span class=\"sjis\">b</span><span class=\"sjis\">c</span>[/sjis]</p>\n"
        );
        assert_eq!(
            render("[sjis]\n[sjis]\na\n[/sjis]\n[/sjis]"),
            "<p>[sjis]<span class=\"sjis\">a</span>[/sjis]</p>\n"
        );

        // An empty inner block is skipped and doesn't make the outer one match
        assert_eq!(render("[sjis]a[sjis][/sjis][/sjis]"), "<p>[sjis]a[sjis][/sjis][/sjis]</p>\n");
    }

    #[test]
    fn unmatched_sjis() {
        assert_eq!(render("[sjis]a"), "<p>[sjis]a</p>\n");
        assert_eq!(render("a[/sjis]"), "<p>a[/sjis]</p>\n");
        assert_eq!(render("[/sjis]a[sjis]"), "<p>[/sjis]a[sjis]</p>\n");
        assert_eq!(render("[sjis]a\n\nb"), "<p>[sjis]a</p>\n<p>b</p>\n");
        assert_eq!(render("[sjis]é"), "<p>[sjis]é</p>\n");
        assert_eq!(render("é[/sjis]"), "<p>é[/sjis]</p>\n");

        // Incomplete tags don't count
        assert_eq!(render("[sjis a[/sjis]"), "<p>[sjis a[/sjis]</p>\n");
        assert_eq!(render("[sjis]a[/sjis"), "<p>[sjis]a[/sjis</p>\n");

        // Extra tags next to a matched block stay literal
        assert_eq!(render("[sjis]a[/sjis][/sjis]"), "<p><span class=\"sjis\">a</span>[/sjis]</p>\n");
        assert_eq!(render("[sjis][sjis]a[/sjis]"), "<p>[sjis]<span class=\"sjis\">a</span></p>\n");
        assert_eq!(render("[/sjis][sjis]a[/sjis]"), "<p>[/sjis]<span class=\"sjis\">a</span></p>\n");
        assert_eq!(render("[sjis]a[/sjis][sjis]b"), "<p><span class=\"sjis\">a</span>[sjis]b</p>\n");
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
