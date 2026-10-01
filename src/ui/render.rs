//! A frame of styled cells, and drawing it to the terminal: only rows that
//! changed since the previous frame are rewritten.
use crate::theme::{to_256, ColorMode, Rgb, Theme};
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style { Text, Muted, Title, Cap, Header }

/// `ch` is "" for the right half of a wide character.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell { pub ch: String, pub style: Style }

#[derive(Clone, Debug, PartialEq)]
pub struct Frame { pub w: usize, pub h: usize, pub rows: Vec<Vec<Cell>> }

impl Frame {
    pub fn new(w: usize, h: usize) -> Frame {
        Frame { w, h, rows: vec![vec![Cell { ch: " ".into(), style: Style::Text }; w]; h] }
    }

    /// Text from x on row y, never reaching `limit` (exclusive) or past the
    /// frame; zero-width and control characters are skipped. Returns the next x.
    pub fn put(&mut self, x: usize, y: usize, text: &str, style: Style, limit: usize) -> usize {
        let limit = limit.min(self.w);
        let mut x = x;
        if y >= self.h { return x; }
        for c in text.chars() {
            let cw = c.width().unwrap_or(0);
            if cw == 0 { continue; }
            if x + cw > limit { break; }
            self.rows[y][x] = Cell { ch: c.to_string(), style };
            if cw == 2 { self.rows[y][x + 1] = Cell { ch: String::new(), style }; }
            x += cw;
        }
        x
    }

    pub fn text(&self) -> String {
        self.rows.iter()
            .map(|r| r.iter().map(|c| c.ch.as_str()).collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>().join("\n")
    }
}

pub struct Term { theme: Theme, mode: ColorMode, prev: Option<Frame> }

impl Term {
    pub fn new(theme: Theme, mode: ColorMode) -> Term { Term { theme, mode, prev: None } }

    pub fn invalidate(&mut self) { self.prev = None; }

    fn color(&self, c: Rgb) -> Color {
        match self.mode { ColorMode::True => Color::Rgb { r: c.0, g: c.1, b: c.2 }, _ => Color::AnsiValue(to_256(c)) }
    }

    fn colors(&self, s: Style) -> (Rgb, Rgb, bool) {
        let t = &self.theme;
        match s {
            Style::Text => (t.fg, t.bg, false),
            Style::Muted => (t.muted, t.bg, false),
            Style::Title => (t.accent, t.bg, true),
            Style::Cap => (t.fg, t.track, false),
            Style::Header => (t.fg, t.bg, true),
        }
    }

    pub fn draw(&mut self, out: &mut impl Write, f: &Frame) -> io::Result<()> {
        let prev = self.prev.take().filter(|p| p.w == f.w && p.h == f.h);
        for (y, row) in f.rows.iter().enumerate() {
            if prev.as_ref().is_some_and(|p| p.rows[y] == *row) { continue; }
            queue!(out, MoveTo(0, y as u16))?;
            let mut cur = None;
            for cell in row.iter().filter(|c| !c.ch.is_empty()) {
                if cur != Some(cell.style) {
                    let (fg, bg, bold) = self.colors(cell.style);
                    queue!(out, SetAttribute(Attribute::Reset))?;
                    if self.mode == ColorMode::None {
                        if cell.style == Style::Muted { queue!(out, SetAttribute(Attribute::Dim))?; }
                    } else {
                        queue!(out, SetForegroundColor(self.color(fg)), SetBackgroundColor(self.color(bg)))?;
                    }
                    if bold { queue!(out, SetAttribute(Attribute::Bold))?; }
                    cur = Some(cell.style);
                }
                queue!(out, Print(&cell.ch))?;
            }
        }
        queue!(out, SetAttribute(Attribute::Reset))?;
        out.flush()?;
        self.prev = Some(f.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::catppuccin;

    #[test]
    fn put_clips_and_handles_wide_chars() {
        let mut f = Frame::new(5, 1);
        assert_eq!(f.put(0, 0, "日本語", Style::Text, 5), 4);
        assert_eq!(f.text(), "日本");
        assert_eq!(f.rows[0][1].ch, "");
        let mut f = Frame::new(10, 1);
        assert_eq!(f.put(2, 0, "abcdef", Style::Text, 5), 5);
        assert_eq!(f.text(), "  abc");
        assert_eq!(f.put(0, 3, "x", Style::Text, 10), 0); // row out of range: no panic
    }

    #[test]
    fn draw_colours_and_diff() {
        let mut f = Frame::new(10, 2);
        f.put(0, 0, " Ctrl ", Style::Cap, 10);
        let mut term = Term::new(catppuccin(), ColorMode::True);
        let mut b1 = Vec::new();
        term.draw(&mut b1, &f).unwrap();
        assert!(String::from_utf8_lossy(&b1).contains("48;2;69;71;90"));   // track #45475a
        let mut b2 = Vec::new();
        term.draw(&mut b2, &f).unwrap();
        assert!(b2.len() < 10, "unchanged frame must not be redrawn: {:?}", String::from_utf8_lossy(&b2));
        term.invalidate();
        let mut b3 = Vec::new();
        term.draw(&mut b3, &f).unwrap();
        assert!(b3.len() > 10);
    }

    #[test]
    fn no_color_mode_emits_no_colours() {
        let mut f = Frame::new(10, 1);
        f.put(0, 0, "x", Style::Muted, 10);
        let mut term = Term::new(catppuccin(), ColorMode::None);
        let mut b = Vec::new();
        term.draw(&mut b, &f).unwrap();
        let s = String::from_utf8_lossy(&b);
        assert!(!s.contains("38;") && !s.contains("48;"));
    }
}
