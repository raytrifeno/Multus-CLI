use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

use crate::cli::MenuItem;
use crate::types::{PdfToolError, Result};
use crate::ui::banner::{brand_name, brand_tagline, multus_orange};

const TWO_COLUMN_BREAKPOINT: u16 = 92;
const COLUMN_GAP: u16 = 4;

fn fit_line(value: &str, width: u16) -> String {
    let usable_width = usize::from(width.saturating_sub(1));
    if usable_width == 0 {
        return String::new();
    }

    value.chars().take(usable_width).collect()
}

fn divider(width: u16) -> String {
    let usable_width = usize::from(width.saturating_sub(1)).max(1);
    "-".repeat(usable_width)
}

fn queue_line<W: Write>(
    stdout: &mut W,
    row: &mut u16,
    width: u16,
    value: &str,
    color: Option<Color>,
    bold: bool,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, *row), Clear(ClearType::CurrentLine))?;
    if let Some(color) = color {
        queue!(stdout, SetForegroundColor(color))?;
    }
    if bold {
        queue!(stdout, SetAttribute(Attribute::Bold))?;
    }
    queue!(stdout, Print(fit_line(value, width)))?;
    if bold {
        queue!(stdout, SetAttribute(Attribute::Reset))?;
    }
    if color.is_some() {
        queue!(stdout, ResetColor)?;
    }
    *row = row.saturating_add(1);
    Ok(())
}

fn queue_menu_cell<W: Write>(
    stdout: &mut W,
    x: u16,
    row: u16,
    width: u16,
    index: usize,
    title: &str,
    selected: bool,
) -> io::Result<()> {
    let marker = if selected { ">" } else { " " };
    let label = format!("{marker} {:02}  {title}", index + 1);

    queue!(stdout, MoveTo(x, row))?;
    if selected {
        queue!(
            stdout,
            SetForegroundColor(multus_orange()),
            SetAttribute(Attribute::Bold)
        )?;
    }
    queue!(stdout, Print(fit_line(&label, width)))?;
    if selected {
        queue!(stdout, SetAttribute(Attribute::Reset), ResetColor)?;
    }
    Ok(())
}

fn menu_columns(width: u16) -> usize {
    if width >= TWO_COLUMN_BREAKPOINT { 2 } else { 1 }
}

fn render_menu_grid<W: Write>(
    stdout: &mut W,
    row: &mut u16,
    width: u16,
    menu_items: &[MenuItem],
    selected_index: usize,
) -> io::Result<()> {
    let columns = menu_columns(width);
    let rows = menu_items.len().div_ceil(columns);
    let usable_width = width.saturating_sub(1);
    let column_width = if columns == 2 {
        usable_width.saturating_sub(COLUMN_GAP) / 2
    } else {
        usable_width
    };

    for row_index in 0..rows {
        queue!(stdout, MoveTo(0, *row), Clear(ClearType::CurrentLine))?;

        if let Some((title, _description, _command)) = menu_items.get(row_index) {
            queue_menu_cell(
                stdout,
                0,
                *row,
                column_width,
                row_index,
                title,
                row_index == selected_index,
            )?;
        }

        if columns == 2 {
            let right_index = row_index + rows;
            if let Some((title, _description, _command)) = menu_items.get(right_index) {
                queue_menu_cell(
                    stdout,
                    column_width + COLUMN_GAP,
                    *row,
                    column_width,
                    right_index,
                    title,
                    right_index == selected_index,
                )?;
            }
        }

        *row = row.saturating_add(1);
    }

    Ok(())
}

fn render_arrow_menu(
    menu_items: &[MenuItem],
    selected_index: usize,
    version_line: &str,
) -> Result<()> {
    let mut stdout = io::stdout();
    let (width, _) = terminal::size().unwrap_or((80, 24));
    let mut row = 0u16;
    let selected_item = menu_items
        .get(selected_index)
        .ok_or_else(|| PdfToolError::new("Selected menu item is out of range."))?;

    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All), Hide)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue_line(
        &mut stdout,
        &mut row,
        width,
        brand_name(),
        Some(multus_orange()),
        true,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        brand_tagline(),
        Some(Color::DarkGrey),
        false,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        &divider(width),
        Some(Color::DarkGrey),
        false,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        "Arrows: move   Enter: run   Esc: split",
        Some(Color::DarkGrey),
        false,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        "QQ: back from prompt   Q twice: quit",
        Some(Color::DarkGrey),
        false,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(&mut stdout, &mut row, width, "", None, false)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        "COMMANDS",
        Some(Color::DarkGrey),
        true,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    render_menu_grid(&mut stdout, &mut row, width, menu_items, selected_index)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue_line(&mut stdout, &mut row, width, "", None, false)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        &format!("READY  {}", selected_item.0),
        Some(multus_orange()),
        true,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        selected_item.1,
        Some(Color::DarkGrey),
        false,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        &format!("Command: multus {}", selected_item.2),
        Some(Color::DarkGrey),
        false,
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(&mut stdout, &mut row, width, "", None, false)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue_line(
        &mut stdout,
        &mut row,
        width,
        version_line,
        Some(multus_orange()),
        false,
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
    menu_items: &[MenuItem],
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
                    KeyCode::Enter => return Ok(Some(menu_items[selected].2.to_string())),
                    KeyCode::Esc => return Ok(Some(menu_items[0].2.to_string())),
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
    use super::{fit_line, menu_columns};

    #[test]
    fn fit_line_keeps_one_column_margin() {
        assert_eq!(fit_line("1234567890", 6), "12345");
    }

    #[test]
    fn fit_line_handles_zero_width() {
        assert_eq!(fit_line("abc", 0), "");
    }

    #[test]
    fn wide_terminal_uses_two_columns() {
        assert_eq!(menu_columns(100), 2);
    }

    #[test]
    fn narrow_terminal_uses_single_column() {
        assert_eq!(menu_columns(80), 1);
    }
}
