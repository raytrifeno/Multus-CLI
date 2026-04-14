use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

use crate::types::{PdfToolError, Result};
use crate::ui::banner::{multus_orange, queue_multus_logo};

fn render_arrow_menu(
    menu_items: &[(&str, &str)],
    selected_index: usize,
    version_line: &str,
    selected_prefix: &str,
) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue_multus_logo(&mut stdout)
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    queue!(
        stdout,
        Print("Use ↑/↓ to move, Enter to select, Esc for default Split.\nType QQ in any prompt to return here.\n")
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    queue!(stdout, Print("\n"))
        .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;

    let orange = multus_orange();
    for (index, (label, _command)) in menu_items.iter().enumerate() {
        let numbered = format!("{}. {label}", index + 1);
        if index == selected_index {
            queue!(
                stdout,
                SetForegroundColor(orange),
                SetAttribute(Attribute::Bold),
                Print(format!("{selected_prefix} {numbered}\n")),
                SetAttribute(Attribute::Reset),
                ResetColor
            )
            .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
        } else {
            queue!(stdout, Print(format!("  {numbered}\n")))
                .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
        }
    }

    queue!(
        stdout,
        Print("\n"),
        Print(format!("{version_line}\n")),
        Print("Quit: press Q twice.\n")
    )
    .map_err(|e| PdfToolError::new(format!("Failed to draw menu: {e}")))?;
    stdout
        .flush()
        .map_err(|e| PdfToolError::new(format!("Failed to flush menu: {e}")))?;
    Ok(())
}

fn should_use_alternate_screen() -> bool {
    if std::env::var("MULTUS_NO_ALT_SCREEN")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return false;
    }
    !cfg!(target_os = "linux")
}

fn selected_prefix_symbol() -> &'static str {
    if cfg!(target_os = "linux") {
        ">"
    } else {
        "❯"
    }
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
    let selected_prefix = selected_prefix_symbol();
    let menu_result = (|| -> Result<Option<String>> {
        loop {
            render_arrow_menu(menu_items, selected, version_line, selected_prefix)?;

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
