use std::fmt::Write;

use mdbook::book::Book;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

pub struct Ansi;

impl Ansi {
    fn highlight_chapter(content: String) -> mdbook::errors::Result<String> {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);

        let mut ansi_block_spans = vec![];

        let events = Parser::new_ext(&content, opts);
        for (e, span) in events.into_offset_iter() {
            if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(code))) = e {
                if &*code == "ansi" {
                    ansi_block_spans.push(span);
                }
            }
        }

        let mut formatted_content = String::new();
        let mut previous_end = 0;
        for span in ansi_block_spans {
            write!(
                formatted_content,
                "{}<pre class=\"ansi\"><code>{}</code></pre>",
                &content[previous_end..span.start],
                Self::highlight_block(
                    &content[span.start + "```ansi\n".len()..span.end - "```".len()]
                )?,
            )
            .unwrap();
            previous_end = span.end;
        }
        write!(formatted_content, "{}", &content[previous_end..]).unwrap();

        Ok(formatted_content)
    }

    fn highlight_block(content: &str) -> mdbook::errors::Result<String> {
        let mut new_content = String::new();

        enum ParseState {
            Text,
            Backslash,
            HexadecimalLeader(u8),
            OctalLeader(u8),
            Sequence(String),
        }

        let mut ansi_state = AnsiState::default();
        let mut parser_state = ParseState::Text;
        new_content.push_str("<span>");
        for c in content.chars() {
            match parser_state {
                ParseState::Text if c == '\\' => parser_state = ParseState::Backslash,
                ParseState::Text => new_content.push(c),
                ParseState::Backslash if c == 'x' => {
                    parser_state = ParseState::HexadecimalLeader(0)
                }
                ParseState::Backslash if c == '0' => parser_state = ParseState::OctalLeader(0),
                ParseState::Backslash => {
                    parser_state = ParseState::Text;
                    new_content.push(c)
                }
                ParseState::HexadecimalLeader(0) if c == '1' => {
                    parser_state = ParseState::HexadecimalLeader(1)
                }
                ParseState::HexadecimalLeader(0) => {
                    parser_state = ParseState::Text;
                    new_content.push('x');
                    new_content.push(c)
                }
                ParseState::HexadecimalLeader(1) if c == 'b' => {
                    parser_state = ParseState::HexadecimalLeader(2)
                }
                ParseState::HexadecimalLeader(1) => {
                    parser_state = ParseState::Text;
                    new_content.push_str("x1");
                    new_content.push(c)
                }
                ParseState::HexadecimalLeader(2) if c == '[' => {
                    parser_state = ParseState::Sequence(String::new())
                }
                ParseState::HexadecimalLeader(2) => {
                    parser_state = ParseState::Text;
                    new_content.push_str("x1b");
                    new_content.push(c)
                }
                ParseState::HexadecimalLeader(_) => unreachable!(),
                ParseState::OctalLeader(0) if c == '3' => {
                    parser_state = ParseState::HexadecimalLeader(1)
                }
                ParseState::OctalLeader(0) => {
                    parser_state = ParseState::Text;
                    new_content.push('0');
                    new_content.push(c)
                }
                ParseState::OctalLeader(1) if c == '3' => {
                    parser_state = ParseState::HexadecimalLeader(2)
                }
                ParseState::OctalLeader(1) => {
                    parser_state = ParseState::Text;
                    new_content.push_str("03");
                    new_content.push(c)
                }
                ParseState::OctalLeader(2) if c == '[' => {
                    parser_state = ParseState::Sequence(String::new())
                }
                ParseState::OctalLeader(2) => {
                    parser_state = ParseState::Text;
                    new_content.push_str("033");
                    new_content.push(c)
                }
                ParseState::OctalLeader(_) => unreachable!(),
                ParseState::Sequence(ref mut s) if c.is_ascii_digit() || c == ';' => s.push(c),
                ParseState::Sequence(s) if c == 'm' => {
                    new_content.push_str("</span>");
                    ansi_state.update_from_escape_sequence(&s);
                    write!(new_content, "<span style=\"{}\">", ansi_state.css()).unwrap();
                    parser_state = ParseState::Text
                }
                ParseState::Sequence(_) => parser_state = ParseState::Text,
            }
        }
        new_content.push_str("</span>");
        Ok(new_content)
    }
}

impl Preprocessor for Ansi {
    fn name(&self) -> &str {
        "ansi"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> mdbook::errors::Result<Book> {
        let mut res = None;
        book.for_each_mut(|item: &mut BookItem| {
            if let Some(Err(_)) = res {
                return;
            }

            if let BookItem::Chapter(ref mut chapter) = *item {
                res = Some(Ansi::highlight_chapter(chapter.content.clone()).map(|md| {
                    chapter.content = md;
                }));
            }
        });

        res.unwrap_or(Ok(())).map(|_| book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

#[derive(Default)]
struct AnsiState {
    fg: Option<AnsiColour>,
    bg: Option<AnsiColour>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

enum AnsiColour {
    ColourId(u8),
    TrueColour(u8, u8, u8),
}

impl AnsiState {
    fn update_from_escape_sequence(&mut self, sequence: &str) {
        enum ParseState {
            Normal,
            FgColour,
            BgColour,
            Fg256Colour,
            Bg256Colour,
            FgTrueColour(Option<u8>, Option<u8>),
            BgTrueColour(Option<u8>, Option<u8>),
        }

        let mut parser_state = ParseState::Normal;
        for item in sequence.split(';').map(|item| item.parse::<u8>().unwrap()) {
            match parser_state {
                ParseState::Normal => match item {
                    0 => *self = Self::default(),
                    1 => self.bold = true,
                    3 => self.italic = true,
                    4 => self.underline = true,
                    9 => self.strikethrough = true,
                    22 => self.bold = false,
                    23 => self.italic = false,
                    24 => self.underline = false,
                    29 => self.strikethrough = false,
                    30 => self.fg = Some(AnsiColour::ColourId(0)),
                    31 => self.fg = Some(AnsiColour::ColourId(1)),
                    32 => self.fg = Some(AnsiColour::ColourId(2)),
                    33 => self.fg = Some(AnsiColour::ColourId(3)),
                    34 => self.fg = Some(AnsiColour::ColourId(4)),
                    35 => self.fg = Some(AnsiColour::ColourId(5)),
                    36 => self.fg = Some(AnsiColour::ColourId(6)),
                    37 => self.fg = Some(AnsiColour::ColourId(7)),
                    38 => parser_state = ParseState::FgColour,
                    39 => self.fg = None,
                    40 => self.bg = Some(AnsiColour::ColourId(0)),
                    41 => self.bg = Some(AnsiColour::ColourId(1)),
                    42 => self.bg = Some(AnsiColour::ColourId(2)),
                    43 => self.bg = Some(AnsiColour::ColourId(3)),
                    44 => self.bg = Some(AnsiColour::ColourId(4)),
                    45 => self.bg = Some(AnsiColour::ColourId(5)),
                    46 => self.bg = Some(AnsiColour::ColourId(6)),
                    47 => self.bg = Some(AnsiColour::ColourId(7)),
                    48 => parser_state = ParseState::BgColour,
                    49 => self.bg = None,
                    90 => self.fg = Some(AnsiColour::ColourId(8)),
                    91 => self.fg = Some(AnsiColour::ColourId(9)),
                    92 => self.fg = Some(AnsiColour::ColourId(10)),
                    93 => self.fg = Some(AnsiColour::ColourId(11)),
                    94 => self.fg = Some(AnsiColour::ColourId(12)),
                    95 => self.fg = Some(AnsiColour::ColourId(13)),
                    96 => self.fg = Some(AnsiColour::ColourId(14)),
                    97 => self.fg = Some(AnsiColour::ColourId(15)),
                    100 => self.bg = Some(AnsiColour::ColourId(8)),
                    101 => self.bg = Some(AnsiColour::ColourId(9)),
                    102 => self.bg = Some(AnsiColour::ColourId(10)),
                    103 => self.bg = Some(AnsiColour::ColourId(11)),
                    104 => self.bg = Some(AnsiColour::ColourId(12)),
                    105 => self.bg = Some(AnsiColour::ColourId(13)),
                    106 => self.bg = Some(AnsiColour::ColourId(14)),
                    107 => self.bg = Some(AnsiColour::ColourId(15)),
                    _ => {}
                },
                ParseState::FgColour => match item {
                    2 => parser_state = ParseState::FgTrueColour(None, None),
                    5 => parser_state = ParseState::Fg256Colour,
                    _ => {}
                },
                ParseState::BgColour => match item {
                    2 => parser_state = ParseState::BgTrueColour(None, None),
                    5 => parser_state = ParseState::Bg256Colour,
                    _ => {}
                },
                ParseState::Fg256Colour => {
                    self.fg = Some(AnsiColour::ColourId(item));
                    parser_state = ParseState::Normal;
                }
                ParseState::Bg256Colour => {
                    self.bg = Some(AnsiColour::ColourId(item));
                    parser_state = ParseState::Normal;
                }
                ParseState::FgTrueColour(ref mut next @ None, _)
                | ParseState::BgTrueColour(ref mut next @ None, _)
                | ParseState::FgTrueColour(_, ref mut next @ None)
                | ParseState::BgTrueColour(_, ref mut next @ None) => *next = Some(item),
                ParseState::FgTrueColour(Some(red), Some(green)) => {
                    self.fg = Some(AnsiColour::TrueColour(red, green, item));
                    parser_state = ParseState::Normal;
                }
                ParseState::BgTrueColour(Some(red), Some(green)) => {
                    self.bg = Some(AnsiColour::TrueColour(red, green, item));
                    parser_state = ParseState::Normal;
                }
            }
        }
    }

    fn css(&self) -> String {
        let mut css = String::new();
        if let Some(ref fg) = self.fg {
            write!(css, "color: {};", fg.css()).unwrap();
        }
        if let Some(ref bg) = self.bg {
            write!(css, "background-color: {};", bg.css()).unwrap();
        }
        if self.bold {
            write!(css, "font-weight: bold;").unwrap();
        }
        if self.italic {
            write!(css, "font-style: italic;").unwrap();
        }
        match (self.underline, self.strikethrough) {
            (true, true) => write!(css, "text-decoration: underline line-through;").unwrap(),
            (true, _) => write!(css, "text-decoration: underline;").unwrap(),
            (_, true) => write!(css, "text-decoration: line-through;").unwrap(),
            (_, _) => {}
        }
        css
    }
}

impl AnsiColour {
    fn css(&self) -> String {
        match self {
            AnsiColour::ColourId(0) => "black".to_owned(),
            AnsiColour::ColourId(1) => "maroon".to_owned(),
            AnsiColour::ColourId(2) => "green".to_owned(),
            AnsiColour::ColourId(3) => "olive".to_owned(),
            AnsiColour::ColourId(4) => "navy".to_owned(),
            AnsiColour::ColourId(5) => "purple".to_owned(),
            AnsiColour::ColourId(6) => "teal".to_owned(),
            AnsiColour::ColourId(7) => "grey".to_owned(),
            AnsiColour::ColourId(8) => "silver".to_owned(),
            AnsiColour::ColourId(9) => "red".to_owned(),
            AnsiColour::ColourId(10) => "lime".to_owned(),
            AnsiColour::ColourId(11) => "yellow".to_owned(),
            AnsiColour::ColourId(12) => "blue".to_owned(),
            AnsiColour::ColourId(13) => "fuschia".to_owned(),
            AnsiColour::ColourId(14) => "aqua".to_owned(),
            AnsiColour::ColourId(15) => "white".to_owned(),
            AnsiColour::ColourId(id @ 16..=231) => {
                // Adapted from https://gist.github.com/MightyPork/1d9bd3a3fd4eb1a661011560f6921b5b
                let n = id - 16;
                let b = n % 6;
                let g = (n - b) / 6 % 6;
                let r = (n - b - g * 6) / 36 % 6;
                format!(
                    "rgb({},{},{})",
                    if r == 0 { 0 } else { r * 40 + 55 },
                    if g == 0 { 0 } else { g * 40 + 55 },
                    if b == 0 { 0 } else { b * 40 + 55 },
                )
            }
            AnsiColour::ColourId(id @ 232..=255) => {
                format!("rgb({0},{0},{0})", (id - 232) * 10 + 8)
            }
            AnsiColour::TrueColour(r, g, b) => format!("rgb({},{},{})", r, g, b),
        }
    }
}
