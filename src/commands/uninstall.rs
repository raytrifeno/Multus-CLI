use std::io::{self, IsTerminal, Write};

use crate::cli::UninstallArgs;
use crate::types::{PdfToolError, Result};
use crate::updater::uninstall_multus;

pub(crate) fn handle_uninstall(args: UninstallArgs) -> Result<i32> {
    if !args.yes && io::stdin().is_terminal() {
        print!("Uninstall Multus from this machine? Type YES to continue: ");
        io::stdout()
            .flush()
            .map_err(|e| PdfToolError::new(format!("Failed to flush stdout: {e}")))?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|e| PdfToolError::new(format!("Failed to read confirmation: {e}")))?;
        if answer.trim() != "YES" {
            println!("Uninstall cancelled.");
            return Ok(0);
        }
    }

    let status = uninstall_multus()?;
    println!("{status}");
    Ok(0)
}
