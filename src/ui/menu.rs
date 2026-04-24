use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

use crate::types::{PdfToolError, Result};
use crate::ui::banner::{multus_logo_lines, multus_orange};

fn fit_line(value: &str, width: u16) -> String {
    let usable_width = usize::from(width.saturating_sub(1));
    if usable_width == 0 {
        return String::new();
    }

    value.chars().take(usable_width).collect()
}

fn queue_line<W: Write>(stdout: &mut W, row: &mut u16, width: u16, value: &str) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(0, *row),
        Clear(ClearType::CurrentLine),
        Print(fit_line(value, width))
    )?;
    *row = row.saturating_add(1);
    Ok(())
}

fn render_arrow_menu(
    menu_items: &[(&str, &str)],
    selected_index: usize,
    version_line: &str,
) -> Result<()> {
    let mut stdout = io::stdout();
    let (width, _) = terminal::size().unwrap_or((80, 24));
    let orange = multus_orange();
    let mut row = 0u16;

    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All), Hide)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue!(
        stdout,
        SetForegroundColor(orange),
        SetAttribute(Attribute::Bold)
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    for line in multus_logo_lines() {
        queue_line(&mut stdout, &mut row, width, line)
            .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    }

    queue!(stdout, ResetColor, SetAttribute(Attribute::Reset),)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue_line(&mut stdout, &mut row, width, "")
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue!(
        stdout,
        MoveTo(0, row),
        SetAttribute(Attribute::Bold),
        Clear(ClearType::CurrentLine),
        Print(fit_line("Multus document tools", width)),
        SetAttribute(Attribute::Reset),
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    row = row.saturating_add(1);

    queue_line(&mut stdout, &mut row, width, "Up/Down: move  Enter: select")
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        "Esc: Split  QQ: back from prompt  Q twice: quit",
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue_line(&mut stdout, &mut row, width, "")
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(&mut stdout, &mut row, width, "Commands")
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    for (index, (label, _command)) in menu_items.iter().enumerate() {
        let marker = if index == selected_index { ">" } else { " " };
        let line = format!("{marker} {:02}. {label}", index + 1);

        if index == selected_index {
            queue!(
                stdout,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(orange),
                SetAttribute(Attribute::Bold),
                Print(fit_line(&line, width)),
                SetAttribute(Attribute::Reset),
                ResetColor,
            )
            .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
        } else {
            queue_line(&mut stdout, &mut row, width, &line)
                .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
            continue;
        }
        row = row.saturating_add(1);
    }

    queue_line(&mut stdout, &mut row, width, "")
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue!(
        stdout,
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        SetForegroundColor(orange),
        Print(fit_line(version_line, width)),
        ResetColor,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    stdout
        .flush()
        .map_err(|e| PdfToolError::new(format!("Failed to flush menu: {e}")))?;
    Ok(())
}

fn should_use_alternate_screen() -> bool {
    !std::env::var("MULTUS_NO_ALT_SCREEN")
        .map(|v| v == "1")
        .unwrap_or(false)
}

pub(crate) fn choose_command_with_arrows(
    menu_items: &[(&str, &str)],
    version_line: &str,
) -> Result<Option<String>> {
    if menu_items.is_empty() {
        return Err(PdfToolError::new("Menu options are empty."));
    }

    terminal::enable_raw_mode()
        .map_err(|e| PdfToolError::new(format!("Failed to enable raw mode: {e}")))?;
    let mut stdout = io::stdout();
    let use_alternate_screen = should_use_alternate_screen();
    if use_alternate_screen {
        execute!(
            stdout,
            EnterAlternateScreen,
            MoveTo(0, 0),
            Clear(ClearType::All),
            Hide
        )
        .map_err(|e| PdfToolError::new(format!("Failed to initialize interactive menu: {e}")))?;
    } else {
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All), Hide).map_err(|e| {
            PdfToolError::new(format!("Failed to initialize interactive menu: {e}"))
        })?;
    }

    let mut selected = 0usize;
    let mut q_press_count = 0u8;
    let menu_result = (|| -> Result<Option<String>> {
        loop {
            render_arrow_menu(menu_items, selected, version_line)?;

            let evt = event::read()
                .map_err(|e| PdfToolError::new(format!("Failed to read keyboard input: {e}")))?;
            if let Event::Key(key_event) = evt {
                if key_event.kind == KeyEventKind::Release {
                    continue;
                }

                match key_event.code {
                    KeyCode::Up => {
                        q_press_count = 0;
                        if selected == 0 {
                            selected = menu_items.len() - 1;
                        } else {
                            selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        q_press_count = 0;
                        selected = (selected + 1) % menu_items.len();
                    }
                    KeyCode::Enter => return Ok(Some(menu_items[selected].1.to_string())),
                    KeyCode::Esc => return Ok(Some(menu_items[0].1.to_string())),
                    KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'q') => {
                        q_press_count += 1;
                        if q_press_count >= 2 {
                            return Ok(None);
                        }
                    }
                    _ => {
                        q_press_count = 0;
                    }
                }
            }
        }
    })();

    if use_alternate_screen {
        let _ = execute!(
            stdout,
            Show,
            ResetColor,
            SetAttribute(Attribute::Reset),
            LeaveAlternateScreen
        );
    } else {
        let _ = execute!(stdout, Show, ResetColor, SetAttribute(Attribute::Reset));
    }
    let _ = terminal::disable_raw_mode();
    menu_result
}

#[cfg(test)]
mod tests {
    use super::fit_line;

    #[test]
    fn fit_line_keeps_one_column_margin() {
        assert_eq!(fit_line("1234567890", 6), "12345");
    }

    #[test]
    fn fit_line_handles_zero_width() {
        assert_eq!(fit_line("abc", 0), "");
    }
}
