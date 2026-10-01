pub mod input;
pub mod layout;
pub mod render;

use crate::model::Section;
use crate::theme::{ColorMode, Theme};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io;

fn restore() {
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

/// Full-screen sheet until the user quits; the terminal is restored on every
/// exit path, including a panic.
pub fn run(sections: &[Section], apps: &[&str], theme: &Theme, mode: ColorMode, mut st: input::State) -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, Hide)?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| { restore(); hook(info); }));
    let view = layout::View { sections, apps, theme_name: &theme.name, plain: mode == ColorMode::None };
    let mut term = render::Term::new(theme.clone(), mode);
    let result = (|| -> io::Result<()> {
        loop {
            let (w, h) = terminal::size()?;
            let (f, max) = layout::frame(&view, &st, w as usize, h as usize);
            st.scroll = st.scroll.min(max);
            term.draw(&mut out, &f)?;
            match event::read()? {
                Event::Key(k) => if let Some(key) = input::from_event(&k) {
                    input::apply(&mut st, key, apps.len(), (h as usize).saturating_sub(4).max(1));
                },
                Event::Resize(..) => term.invalidate(),
                _ => {}
            }
            if st.quit { return Ok(()); }
        }
    })();
    restore();
    result
}
