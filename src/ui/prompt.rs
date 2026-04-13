use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::types::{PdfToolError, Result};

const CONTROL_BACK_TO_MENU: &str = "__CONTROL_BACK_TO_MENU__";
static INTERACTIVE_MODE: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_interactive_mode(value: bool) {
    INTERACTIVE_MODE.store(value, Ordering::Relaxed);
}

pub(crate) fn is_back_to_menu_error(err: &PdfToolError) -> bool {
    err.0 == CONTROL_BACK_TO_MENU
}

pub(crate) fn prompt_non_empty(prompt: &str) -> Result<String> {
    loop {
        let value = prompt_optional(prompt)?;
        if !value.trim().is_empty() {
            return Ok(value.trim().to_string());
        }
        println!("Input cannot be empty.");
    }
}

pub(crate) fn prompt_optional(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|e| PdfToolError::new(format!("Failed to flush stdout: {e}")))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| PdfToolError::new(format!("Failed to read input: {e}")))?;
    let value = input.trim().to_string();
    if INTERACTIVE_MODE.load(Ordering::Relaxed) && value.eq_ignore_ascii_case("qq") {
        return Err(PdfToolError::new(CONTROL_BACK_TO_MENU));
    }
    Ok(value)
}
