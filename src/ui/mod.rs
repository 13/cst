pub mod input;
pub mod layout;
pub mod render;

use crate::model::Section;
use crate::theme::{ColorMode, Theme};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether we pushed the kitty keyboard protocol (so restore pops it).
static ENHANCED: AtomicBool = AtomicBool::new(false);

fn restore() {
    if ENHANCED.swap(false, Ordering::SeqCst) { let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags); }
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

/// Full-screen sheet until the user quits; the terminal is restored on every
/// exit path, including a panic.
pub fn run(sections: &[Section], apps: &[&str], entries: &[layout::Entry], theme: &Theme, mode: ColorMode, mut st: input::State) -> io::Result<()> {
    let mut out = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, Hide)?;
    // kitty keyboard protocol where supported: Ctrl+Shift+T ≠ Ctrl+T, Ctrl+I ≠ Tab
    if terminal::supports_keyboard_enhancement().unwrap_or(false)
        && execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)).is_ok() {
        ENHANCED.store(true, Ordering::SeqCst);
    }
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| { restore(); hook(info); }));
    let view = layout::View { sections, apps, theme_name: &theme.name, plain: mode == ColorMode::None };
    let mut term = render::Term::new(theme.clone(), mode);
    let result = (|| -> io::Result<()> {
        loop {
            let (w, h) = terminal::size()?;
            let (w, h) = (w as usize, h as usize);
            let f = match st.screen {
                input::Screen::Picker => layout::picker_frame(&view, entries, &st, w, h),
                input::Screen::Sheet => {
                    let (f, max) = layout::frame(&view, &st, w, h);
                    st.scroll = st.scroll.min(max);
                    f
                }
            };
            term.draw(&mut out, &f)?;
            match event::read()? {
                Event::Key(k) => if let Some(key) = input::from_event(&k) {
                    input::note_key(&mut st, &key, std::time::Instant::now());
                    if st.quit { return Ok(()); }
                    match st.screen {
                        input::Screen::Picker => input::apply_picker(&mut st, key, apps, layout::picker_cols(w)),
                        input::Screen::Sheet => input::apply(&mut st, key, apps.len(), h.saturating_sub(4).max(1)),
                    }
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
