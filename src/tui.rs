#![allow(dead_code)]

use std::env;
use std::io::{self, Write};
use std::sync::OnceLock;
use unicode_width::UnicodeWidthChar;

static COLORS_ENABLED: OnceLock<bool> = OnceLock::new();

fn init_colors_enabled() -> bool {
    let no_color = env::var("NO_COLOR").unwrap_or_default();
    let no_colors = env::var("NO_COLORS").unwrap_or_default();
    no_color.is_empty() && no_colors.is_empty()
}

pub fn colors_enabled() -> bool {
    *COLORS_ENABLED.get_or_init(init_colors_enabled)
}

pub fn disable_colors() {
    let _ = COLORS_ENABLED.get_or_init(|| false);
}

pub fn enable_colors() {
    let _ = COLORS_ENABLED.get_or_init(|| true);
}

pub mod ansi {
    use super::colors_enabled;

    pub const ALT_SCREEN_ON: &str = "\x1b[?1049h";
    pub const ALT_SCREEN_OFF: &str = "\x1b[?1049l";
    pub const CURSOR_HIDE: &str = "\x1b[?25l";
    pub const CURSOR_SHOW: &str = "\x1b[?25h";
    pub const CURSOR_HOME: &str = "\x1b[H";
    pub const CLEAR_SCREEN: &str = "\x1b[2J";
    pub const CLEAR_LINE: &str = "\x1b[K";
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const REVERSE: &str = "\x1b[7m";
    pub const REVERSE_OFF: &str = "\x1b[27m";

    pub fn fg(code: u8) -> String {
        if colors_enabled() {
            format!("\x1b[38;5;{}m", code)
        } else {
            String::new()
        }
    }

    pub fn bg(code: u8) -> String {
        if colors_enabled() {
            format!("\x1b[48;5;{}m", code)
        } else {
            String::new()
        }
    }

    pub fn move_col(col: u16) -> String {
        format!("\x1b[{}G", col)
    }

    pub fn sgr(codes: &[&str]) -> String {
        if !colors_enabled() {
            return String::new();
        }
        let joined = codes.join(";");
        format!("\x1b[{}m", joined)
    }

    pub fn set_title(title: &str) -> String {
        format!("\x1b]2;{}\x07", title)
    }
}

pub mod palette {
    use super::ansi;
    use super::colors_enabled;

    pub fn header() -> String {
        if colors_enabled() {
            ansi::sgr(&["1", "38;5;114"])
        } else {
            String::new()
        }
    }

    pub fn accent() -> String {
        if colors_enabled() {
            ansi::sgr(&["1", "38;5;214"])
        } else {
            String::new()
        }
    }

    pub fn highlight() -> String {
        if colors_enabled() {
            "\x1b[1;33m".to_string()
        } else {
            String::new()
        }
    }

    pub fn muted() -> String {
        ansi::fg(245)
    }

    pub fn match_color() -> String {
        if colors_enabled() {
            ansi::sgr(&["1", "38;5;226"])
        } else {
            String::new()
        }
    }

    pub fn input_hint() -> String {
        ansi::fg(244)
    }

    pub fn input_cursor_on() -> String {
        if colors_enabled() {
            "\x1b[7m".to_string()
        } else {
            String::new()
        }
    }

    pub fn input_cursor_off() -> String {
        if colors_enabled() {
            "\x1b[27m".to_string()
        } else {
            String::new()
        }
    }

    pub fn selected_bg() -> String {
        ansi::bg(238)
    }

    pub fn danger_bg() -> String {
        ansi::bg(52)
    }
}

pub struct Terminal;

impl Terminal {
    pub fn size() -> (u16, u16) {
        let width = env::var("TRY_WIDTH")
            .ok()
            .and_then(|w| w.parse::<u16>().ok())
            .filter(|&w| w > 0);

        let height = env::var("TRY_HEIGHT")
            .ok()
            .and_then(|h| h.parse::<u16>().ok())
            .filter(|&h| h > 0);

        if let (Some(w), Some(h)) = (width, height) {
            return (w, h);
        }

        if let Ok((term_width, term_height)) = crossterm::terminal::size() {
            return (width.unwrap_or(term_width), height.unwrap_or(term_height));
        }

        (width.unwrap_or(80), height.unwrap_or(24))
    }
}

#[derive(Debug, Clone)]
pub struct Screen {
    pub width: u16,
    pub height: u16,
    pub header: Section,
    pub body: Section,
    pub footer: Section,
    pub test_no_cls: bool,
}

impl Screen {
    pub fn new(width: u16, height: u16, header_size: usize, footer_size: usize) -> Self {
        Self {
            width,
            height,
            header: Section::new(header_size),
            body: Section::new(usize::MAX),
            footer: Section::new(footer_size),
            test_no_cls: false,
        }
    }

    pub fn flush(&self, io: &mut impl Write) -> io::Result<()> {
        let width = self.width as usize;
        let height = self.height as usize;
        let header_height = self.header.max_lines.min(height);
        let footer_height = self
            .footer
            .max_lines
            .min(height.saturating_sub(header_height));
        let body_height = height.saturating_sub(header_height + footer_height);

        let mut lines = Vec::with_capacity(height);
        let mut cursor_row = None;
        let mut global_row = 1usize;

        self.render_section_into(
            &self.header,
            header_height,
            width,
            &mut lines,
            &mut cursor_row,
            &mut global_row,
        );
        self.render_section_into(
            &self.body,
            body_height,
            width,
            &mut lines,
            &mut cursor_row,
            &mut global_row,
        );
        self.render_section_into(
            &self.footer,
            footer_height,
            width,
            &mut lines,
            &mut cursor_row,
            &mut global_row,
        );

        let mut frame = String::new();
        if !self.test_no_cls {
            frame.push_str(ansi::ALT_SCREEN_ON);
            frame.push_str(ansi::CURSOR_HIDE);
        }
        frame.push_str(ansi::CURSOR_HOME);
        frame.push_str(ansi::CLEAR_SCREEN);
        frame.push_str(&lines.join("\n"));

        if let Some((row, col)) = cursor_row {
            frame.push_str(&format!("\x1b[{};{}H", row, col));
        }

        if !self.test_no_cls {
            frame.push_str(ansi::CURSOR_SHOW);
        }

        io.write_all(frame.as_bytes())
    }

    pub fn flush_stderr(&self) -> io::Result<()> {
        let mut stderr = io::stderr();
        self.flush(&mut stderr)
    }

    pub fn clear(&mut self) {
        self.header.clear();
        self.body.clear();
        self.footer.clear();
    }

    fn render_section_into(
        &self,
        section: &Section,
        section_height: usize,
        width: usize,
        lines: &mut Vec<String>,
        cursor_row: &mut Option<(usize, usize)>,
        global_row: &mut usize,
    ) {
        for idx in 0..section_height {
            if let Some(line) = section.lines.get(idx) {
                if cursor_row.is_none() {
                    if let Some(input_col) = line.input_cursor_col {
                        let col = input_col.clamp(1, width.max(1));
                        *cursor_row = Some((*global_row, col));
                    }
                }
                lines.push(line.render(width));
            } else {
                lines.push(" ".repeat(width));
            }
            *global_row += 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    pub lines: Vec<Line>,
    pub max_lines: usize,
}

impl Section {
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
        }
    }

    pub fn add_line(&mut self) -> &mut Line {
        self.lines.push(Line::new());
        self.lines
            .last_mut()
            .expect("line exists after push in Section::add_line")
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    pub left: SegmentWriter,
    pub center: SegmentWriter,
    pub right: SegmentWriter,
    pub background: Option<u8>,
    pub input_cursor_col: Option<usize>,
}

impl Line {
    pub fn new() -> Self {
        Self {
            left: SegmentWriter::new(),
            center: SegmentWriter::new(),
            right: SegmentWriter::new(),
            background: None,
            input_cursor_col: None,
        }
    }

    pub fn render(&self, width: usize) -> String {
        if width == 0 {
            return String::new();
        }

        let (left_text, left_width) = self.left.render(width);
        let right_max = width.saturating_sub(left_width);
        let (right_text, right_width) = self.right.render(right_max);
        let center_max = width.saturating_sub(left_width + right_width);
        let (center_text, center_width) = self.center.render(center_max);

        let right_start = width.saturating_sub(right_width);
        let center_start = left_width + center_max.saturating_sub(center_width) / 2;

        let mut out = String::new();
        out.push_str(&left_text);

        let mut col = left_width;
        if center_width > 0 {
            if center_start > col {
                out.push_str(&" ".repeat(center_start - col));
                col = center_start;
            }
            out.push_str(&center_text);
            col += center_width;
        }

        if right_start > col {
            out.push_str(&" ".repeat(right_start - col));
            col = right_start;
        }

        if right_width > 0 {
            out.push_str(&right_text);
            col += right_width;
        }

        if col < width {
            out.push_str(&" ".repeat(width - col));
        }

        if let Some(bg_color) = self.background {
            if colors_enabled() {
                return format!("{}{}{}", ansi::bg(bg_color), out, ansi::RESET);
            }
        }

        out
    }
}

#[derive(Debug, Clone)]
pub struct SegmentWriter {
    pub segments: Vec<Segment>,
}

impl SegmentWriter {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn write(&mut self, text: &str, color: &str) {
        self.segments.push(Segment::Text {
            content: text.to_string(),
            style: color.to_string(),
        });
    }

    pub fn write_dim(&mut self, text: &str) {
        self.segments.push(Segment::Text {
            content: text.to_string(),
            style: ansi::DIM.to_string(),
        });
    }

    pub fn write_bold(&mut self, text: &str, color: &str) {
        self.segments.push(Segment::Text {
            content: text.to_string(),
            style: format!("{}{}", ansi::BOLD, color),
        });
    }

    pub fn write_highlight(&mut self, text: &str, bg_color: u8) {
        self.segments.push(Segment::Text {
            content: text.to_string(),
            style: ansi::bg(bg_color),
        });
    }

    pub fn fill(&mut self, ch: char) {
        self.segments.push(Segment::Fill {
            ch,
            style: String::new(),
        });
    }

    pub fn emoji(&mut self, e: &str) {
        self.segments.push(Segment::Emoji {
            emoji: e.to_string(),
        });
    }

    pub fn render(&self, max_width: usize) -> (String, usize) {
        if max_width == 0 {
            return (String::new(), 0);
        }

        let mut out = String::new();
        let mut width = 0;

        for segment in &self.segments {
            if width >= max_width {
                break;
            }

            match segment {
                Segment::Text { content, style } => {
                    let rem = max_width - width;
                    let rendered = if visible_width(content) > rem {
                        truncate(content, rem)
                    } else {
                        content.clone()
                    };
                    let rendered_width = visible_width(&rendered);
                    if rendered_width == 0 {
                        continue;
                    }
                    out.push_str(&apply_style(&rendered, style));
                    width += rendered_width;
                }
                Segment::Fill { ch, style } => {
                    let rem = max_width - width;
                    let filler = repeat_to_width(*ch, rem);
                    if filler.is_empty() {
                        continue;
                    }
                    out.push_str(&apply_style(&filler, style));
                    width += visible_width(&filler);
                }
                Segment::Emoji { emoji } => {
                    let rem = max_width - width;
                    let rendered = if visible_width(emoji) > rem {
                        truncate(emoji, rem)
                    } else {
                        emoji.clone()
                    };
                    let rendered_width = visible_width(&rendered);
                    if rendered_width == 0 {
                        continue;
                    }
                    out.push_str(&rendered);
                    width += rendered_width;
                }
            }
        }

        (out, width)
    }
}

#[derive(Debug, Clone)]
pub enum Segment {
    Text { content: String, style: String },
    Fill { ch: char, style: String },
    Emoji { emoji: String },
}

fn apply_style(text: &str, style: &str) -> String {
    if !colors_enabled() || style.is_empty() {
        return text.to_string();
    }
    format!("{}{}{}", style, text, ansi::RESET)
}

fn repeat_to_width(ch: char, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let unit_width = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
    let count = (width / unit_width) + 2;
    let pattern: String = std::iter::repeat_n(ch, count).collect();
    truncate(&pattern, width)
}

fn ansi_escape_len(s: &str, start: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if start + 2 > bytes.len() || bytes[start] != 0x1b || bytes[start + 1] != b'[' {
        return None;
    }

    let mut idx = start + 2;
    while idx < bytes.len()
        && (bytes[idx].is_ascii_digit() || bytes[idx] == b';' || bytes[idx] == b'?')
    {
        idx += 1;
    }

    if idx < bytes.len() && bytes[idx].is_ascii_alphabetic() {
        Some(idx + 1 - start)
    } else {
        None
    }
}

fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut idx = 0;

    while idx < s.len() {
        if let Some(len) = ansi_escape_len(s, idx) {
            idx += len;
            continue;
        }

        let ch = s[idx..].chars().next().expect("valid UTF-8");
        out.push(ch);
        idx += ch.len_utf8();
    }

    out
}

pub fn visible_width(s: &str) -> usize {
    strip_ansi(s).chars().map(char_width).sum()
}

pub fn truncate(s: &str, max_width: usize) -> String {
    let total = visible_width(s);
    if total <= max_width {
        return s.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    let ellipsis = '…';
    let ellipsis_width = char_width(ellipsis);
    if max_width <= ellipsis_width {
        return ellipsis.to_string();
    }

    let target = max_width - ellipsis_width;
    let mut out = String::new();
    let mut idx = 0;
    let mut width = 0;
    let mut at_capacity = false;

    while idx < s.len() {
        if let Some(len) = ansi_escape_len(s, idx) {
            out.push_str(&s[idx..idx + len]);
            idx += len;
            continue;
        }

        let ch = s[idx..].chars().next().expect("valid UTF-8");
        if !at_capacity {
            let cw = char_width(ch);
            if width + cw > target {
                at_capacity = true;
            } else {
                out.push(ch);
                width += cw;
            }
        }
        idx += ch.len_utf8();
    }

    out.push(ellipsis);
    out
}

pub fn truncate_from_start(s: &str, max_width: usize) -> String {
    let total = visible_width(s);
    if total <= max_width {
        return s.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    let ellipsis = '…';
    let ellipsis_width = char_width(ellipsis);
    if max_width <= ellipsis_width {
        return ellipsis.to_string();
    }

    let keep_width = max_width - ellipsis_width;
    let mut leading_escapes = String::new();
    let mut idx = 0;

    while idx < s.len() {
        if let Some(len) = ansi_escape_len(s, idx) {
            leading_escapes.push_str(&s[idx..idx + len]);
            idx += len;
        } else {
            break;
        }
    }

    let mut skipped = 0;
    let mut out = String::new();
    let to_skip = total.saturating_sub(keep_width);
    idx = 0;

    while idx < s.len() {
        if let Some(len) = ansi_escape_len(s, idx) {
            if skipped >= to_skip {
                out.push_str(&s[idx..idx + len]);
            }
            idx += len;
            continue;
        }

        let ch = s[idx..].chars().next().expect("valid UTF-8");
        let cw = char_width(ch);

        if skipped < to_skip {
            skipped += cw;
        } else {
            out.push(ch);
        }

        idx += ch.len_utf8();
    }

    format!("{}{}{}", leading_escapes, ellipsis, out)
}

#[cfg(test)]
mod tui_tests {
    use super::*;

    struct CountingWriter {
        writes: usize,
        bytes: Vec<u8>,
    }

    impl CountingWriter {
        fn new() -> Self {
            Self {
                writes: 0,
                bytes: Vec::new(),
            }
        }

        fn output(&self) -> String {
            String::from_utf8_lossy(&self.bytes).into_owned()
        }
    }

    impl Write for CountingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn tui_line_left_align_works() {
        let mut line = Line::new();
        line.left.write("abc", "");
        let rendered = line.render(10);
        assert_eq!(visible_width(&rendered), 10);
        assert!(strip_ansi(&rendered).starts_with("abc"));
    }

    #[test]
    fn tui_line_right_align_works() {
        let mut line = Line::new();
        line.right.write("abc", "");
        let rendered = line.render(10);
        assert_eq!(strip_ansi(&rendered), "       abc");
    }

    #[test]
    fn tui_line_center_align_works() {
        let mut line = Line::new();
        line.center.write("ab", "");
        let rendered = line.render(10);
        assert_eq!(strip_ansi(&rendered), "    ab    ");
    }

    #[test]
    fn tui_line_background_fills_full_width() {
        let mut line = Line::new();
        line.left.write("x", "");
        line.background = Some(52);
        let rendered = line.render(8);
        assert_eq!(visible_width(&rendered), 8);
        if colors_enabled() {
            assert!(rendered.starts_with("\x1b[48;5;52m"));
            assert!(rendered.ends_with(ansi::RESET));
        }
    }

    #[test]
    fn tui_screen_renders_frame_with_three_sections() {
        let mut screen = Screen::new(12, 4, 1, 1);
        screen.test_no_cls = true;
        screen.header.add_line().left.write("header", "");
        screen.body.add_line().left.write("body", "");
        screen.footer.add_line().left.write("footer", "");

        let mut writer = CountingWriter::new();
        screen.flush(&mut writer).expect("screen flush succeeds");
        let output = writer.output();
        let plain = strip_ansi(&output);

        assert!(output.starts_with("\x1b[H\x1b[2J"));
        assert!(plain.contains("header"));
        assert!(plain.contains("body"));
        assert!(plain.contains("footer"));
    }

    #[test]
    fn tui_screen_flush_uses_single_write_call() {
        let mut screen = Screen::new(10, 3, 1, 1);
        screen.test_no_cls = true;
        screen.header.add_line().left.write("h", "");

        let mut writer = CountingWriter::new();
        screen.flush(&mut writer).expect("screen flush succeeds");
        assert_eq!(writer.writes, 1);
    }

    #[test]
    fn tui_screen_clear_resets_sections() {
        let mut screen = Screen::new(10, 5, 1, 1);
        screen.header.add_line();
        screen.body.add_line();
        screen.footer.add_line();
        screen.clear();
        assert!(screen.header.lines.is_empty());
        assert!(screen.body.lines.is_empty());
        assert!(screen.footer.lines.is_empty());
    }

    #[test]
    fn visible_width_ascii() {
        assert_eq!(visible_width("hello"), 5);
    }

    #[test]
    fn visible_width_strips_ansi() {
        assert_eq!(visible_width("\x1b[31mhello\x1b[0m"), 5);
    }

    #[test]
    fn visible_width_counts_emoji_as_wide() {
        assert_eq!(visible_width("👋"), 2);
    }

    #[test]
    fn strip_ansi_removes_escape_sequences() {
        assert_eq!(strip_ansi("\x1b[31mhello\x1b[0m"), "hello");
    }

    #[test]
    fn truncate_adds_ellipsis_and_respects_width() {
        let out = truncate("hello world", 5);
        assert!(out.ends_with('…'));
        assert!(visible_width(&out) <= 5);
    }

    #[test]
    fn truncate_preserves_ansi_sequences() {
        let input = "\x1b[31mhello world\x1b[0m";
        let out = truncate(input, 7);
        assert!(out.contains("\x1b[31m"));
        assert!(out.contains("\x1b[0m"));
        assert_eq!(visible_width(&out), 7);
    }

    #[test]
    fn truncate_exact_width_keeps_original() {
        let input = "hello";
        assert_eq!(truncate(input, 5), input);
    }

    #[test]
    fn truncate_from_start_adds_ellipsis_and_respects_width() {
        let out = truncate_from_start("hello world", 6);
        assert!(strip_ansi(&out).starts_with('…'));
        assert!(visible_width(&out) <= 6);
    }

    #[test]
    fn truncate_from_start_preserves_leading_ansi() {
        let input = "\x1b[2mabcdef\x1b[0m";
        let out = truncate_from_start(input, 4);
        assert!(out.starts_with("\x1b[2m"));
        assert!(strip_ansi(&out).starts_with('…'));
        assert_eq!(visible_width(&out), 4);
    }

    #[test]
    fn constants_defined() {
        assert_eq!(ansi::ALT_SCREEN_ON, "\x1b[?1049h");
        assert_eq!(ansi::ALT_SCREEN_OFF, "\x1b[?1049l");
        assert_eq!(ansi::CURSOR_HIDE, "\x1b[?25l");
        assert_eq!(ansi::CURSOR_SHOW, "\x1b[?25h");
        assert_eq!(ansi::CLEAR_SCREEN, "\x1b[2J");
        assert_eq!(ansi::RESET, "\x1b[0m");
        assert_eq!(ansi::BOLD, "\x1b[1m");
    }

    #[test]
    fn terminal_size_fallback() {
        let (w, h) = Terminal::size();
        assert!(w > 0);
        assert!(h > 0);
    }
}

#[derive(Debug, Clone)]
pub struct InputField {
    buffer: String,
    cursor_pos: usize,
}

impl InputField {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.buffer.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    pub fn delete_char_before(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let mut idx = self.cursor_pos.saturating_sub(1);
        while idx > 0 && !self.buffer.is_char_boundary(idx) {
            idx -= 1;
        }

        self.buffer.remove(idx);
        self.cursor_pos = idx;
    }

    pub fn delete_char_at(&mut self) {
        if self.cursor_pos >= self.buffer.len() {
            return;
        }
        self.buffer.remove(self.cursor_pos);
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let mut idx = self.cursor_pos.saturating_sub(1);
        while idx > 0 && !self.buffer.is_char_boundary(idx) {
            idx -= 1;
        }

        self.cursor_pos = idx;
    }

    pub fn move_right(&mut self) {
        if self.cursor_pos >= self.buffer.len() {
            return;
        }

        let mut idx = self.cursor_pos + 1;
        while idx < self.buffer.len() && !self.buffer.is_char_boundary(idx) {
            idx += 1;
        }

        self.cursor_pos = idx.min(self.buffer.len());
    }

    pub fn move_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor_pos = self.buffer.len();
    }

    pub fn kill_to_end(&mut self) {
        self.buffer.truncate(self.cursor_pos);
    }

    pub fn kill_word_backward(&mut self) {
        if self.cursor_pos == 0 {
            return;
        }

        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();

        let cursor_char_idx = chars
            .iter()
            .position(|(byte_pos, _)| *byte_pos == self.cursor_pos)
            .unwrap_or(chars.len());

        let mut pos = cursor_char_idx;

        while pos > 0 {
            if let Some((_, ch)) = chars.get(pos.saturating_sub(1)) {
                if ch.is_ascii_alphanumeric() {
                    break;
                }
                pos -= 1;
            } else {
                break;
            }
        }

        while pos > 0 {
            if let Some((_, ch)) = chars.get(pos.saturating_sub(1)) {
                if !ch.is_ascii_alphanumeric() {
                    break;
                }
                pos -= 1;
            } else {
                break;
            }
        }

        let target_byte_pos = if pos == 0 {
            0
        } else {
            chars.get(pos).map(|(byte_pos, _)| *byte_pos).unwrap_or(0)
        };

        self.buffer.drain(target_byte_pos..self.cursor_pos);
        self.cursor_pos = target_byte_pos;
    }

    pub fn render(&self, writer: &mut SegmentWriter, placeholder: &str, _width: usize) {
        if self.buffer.is_empty() {
            writer.write_dim(placeholder);
            return;
        }

        let mut buf = String::new();
        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();

        for (byte_pos, ch) in &chars {
            if *byte_pos == self.cursor_pos {
                if colors_enabled() {
                    buf.push_str(&palette::input_cursor_on());
                }
                buf.push(*ch);
                if colors_enabled() {
                    buf.push_str(&palette::input_cursor_off());
                }
            } else {
                buf.push(*ch);
            }
        }

        if self.cursor_pos == self.buffer.len() {
            if colors_enabled() {
                buf.push_str(&palette::input_cursor_on());
            }
            buf.push(' ');
            if colors_enabled() {
                buf.push_str(&palette::input_cursor_off());
            }
        }

        writer.write(&buf, "");
    }
}

impl Default for InputField {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod input_field_tests {
    use super::*;

    #[test]
    fn input_field_new_is_empty() {
        let field = InputField::new();
        assert_eq!(field.text(), "");
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_insert_char_advances_cursor() {
        let mut field = InputField::new();
        field.insert_char('a');
        assert_eq!(field.text(), "a");
        assert_eq!(field.cursor_pos, 1);

        field.insert_char('b');
        assert_eq!(field.text(), "ab");
        assert_eq!(field.cursor_pos, 2);
    }

    #[test]
    fn input_field_insert_multibyte_char() {
        let mut field = InputField::new();
        field.insert_char('ö');
        assert_eq!(field.text(), "ö");
        assert_eq!(field.cursor_pos, 2);
    }

    #[test]
    fn input_field_delete_char_before_removes_last_char() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.delete_char_before();
        assert_eq!(field.text(), "a");
        assert_eq!(field.cursor_pos, 1);
    }

    #[test]
    fn input_field_delete_char_before_at_start_is_noop() {
        let mut field = InputField::new();
        field.delete_char_before();
        assert_eq!(field.text(), "");
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_delete_char_at_removes_char_at_cursor() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.insert_char('c');
        field.move_to_start();
        field.delete_char_at();
        assert_eq!(field.text(), "bc");
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_delete_char_at_end_is_noop() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.delete_char_at();
        assert_eq!(field.text(), "a");
        assert_eq!(field.cursor_pos, 1);
    }

    #[test]
    fn input_field_move_left_moves_cursor_back() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.move_left();
        assert_eq!(field.cursor_pos, 1);
        field.move_left();
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_move_left_at_start_is_noop() {
        let mut field = InputField::new();
        field.move_left();
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_move_right_moves_cursor_forward() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.move_to_start();
        field.move_right();
        assert_eq!(field.cursor_pos, 1);
        field.move_right();
        assert_eq!(field.cursor_pos, 2);
    }

    #[test]
    fn input_field_move_right_at_end_is_noop() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.move_right();
        assert_eq!(field.cursor_pos, 1);
    }

    #[test]
    fn input_field_move_to_start() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.move_to_start();
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_move_to_end() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.move_to_start();
        field.move_to_end();
        assert_eq!(field.cursor_pos, 2);
    }

    #[test]
    fn input_field_kill_to_end_truncates() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.insert_char('c');
        field.move_to_start();
        field.move_right();
        field.kill_to_end();
        assert_eq!(field.text(), "a");
        assert_eq!(field.cursor_pos, 1);
    }

    #[test]
    fn input_field_kill_word_backward_skips_non_alnum_then_alnum() {
        let mut field = InputField::new();
        for ch in "hello  world".chars() {
            field.insert_char(ch);
        }
        field.kill_word_backward();
        assert_eq!(field.text(), "hello  ");

        field.kill_word_backward();
        assert_eq!(field.text(), "");
    }

    #[test]
    fn input_field_kill_word_backward_with_punctuation() {
        let mut field = InputField::new();
        for ch in "test-file.txt".chars() {
            field.insert_char(ch);
        }
        field.kill_word_backward();
        assert_eq!(field.text(), "test-file.");

        field.kill_word_backward();
        assert_eq!(field.text(), "test-");

        field.kill_word_backward();
        assert_eq!(field.text(), "");
    }

    #[test]
    fn input_field_kill_word_backward_at_start_is_noop() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.move_to_start();
        field.kill_word_backward();
        assert_eq!(field.text(), "a");
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_clear_empties_buffer() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.clear();
        assert_eq!(field.text(), "");
        assert_eq!(field.cursor_pos, 0);
    }

    #[test]
    fn input_field_render_empty_shows_placeholder() {
        let field = InputField::new();
        let mut writer = SegmentWriter::new();
        field.render(&mut writer, "search...", 50);

        let (rendered, _) = writer.render(50);
        let stripped = strip_ansi(&rendered);
        assert!(stripped.contains("search..."));
    }

    #[test]
    fn input_field_render_with_text_shows_cursor() {
        let mut field = InputField::new();
        field.insert_char('a');
        field.insert_char('b');
        field.insert_char('c');
        field.move_to_start();
        field.move_right();

        let mut writer = SegmentWriter::new();
        field.render(&mut writer, "search...", 50);

        let (rendered, _) = writer.render(50);
        if colors_enabled() {
            assert!(rendered.contains("\x1b[7m"));
            assert!(rendered.contains("\x1b[27m"));
        }
        assert!(rendered.contains('a'));
        assert!(rendered.contains('b'));
        assert!(rendered.contains('c'));
    }

    #[test]
    fn input_field_render_cursor_at_end_shows_reverse_space() {
        let mut field = InputField::new();
        field.insert_char('a');

        let mut writer = SegmentWriter::new();
        field.render(&mut writer, "search...", 50);

        let (rendered, _) = writer.render(50);
        let stripped = strip_ansi(&rendered);
        assert!(stripped.contains('a'));
        if colors_enabled() {
            assert!(rendered.contains("\x1b[7m "));
        }
    }
}
