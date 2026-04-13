mod cli;
mod commands;
mod core;
mod types;
mod ui;
mod update;

use clap::{CommandFactory, Parser};
use cli::{Cli, Commands, menu_items, normalize_argv};
use commands::{
    handle_compress, handle_encrypt, handle_images_to_pdf, handle_merge, handle_reorder,
    handle_split, handle_update, handle_watermark,
};
use std::env;
use std::io::{self, IsTerminal};
use types::Result;
use ui::menu::choose_command_with_arrows;
use update::{UPDATE_REPO_REF, UPDATE_REPO_URL, VersionState, check_version_state};

pub(crate) use core::path::{
    build_output_file_path, ensure_output_dir, has_docx_extension, has_pdf_extension,
    has_supported_image_extension, open_pdf, resolve_input_path,
};
pub(crate) use core::pdf::compress::compress_pdf;
pub(crate) use core::pdf::encrypt::encrypt_pdf;
pub(crate) use core::pdf::images_to_pdf::images_to_pdf;
pub(crate) use core::pdf::merge::merge_pdfs;
pub(crate) use core::pdf::reorder::reorder_pdf;
pub(crate) use core::pdf::split::split_pdf;
pub(crate) use core::pdf::watermark::apply_watermark;
pub(crate) use ui::banner::{print_banner, print_step};
pub(crate) use ui::prompt::{is_back_to_menu_error, prompt_non_empty, prompt_optional};
pub(crate) use ui::spinner::run_with_spinner;

fn interactive_version_line() -> String {
    match check_version_state(UPDATE_REPO_URL, UPDATE_REPO_REF) {
        VersionState::UpdateAvailable { current, latest } => {
            format!("Update tersedia: v{current} -> v{latest} (pilih menu Update)")
        }
        VersionState::UpToDate { current } => format!("Version current: v{current}"),
        VersionState::Unknown { current } => format!("Version current: v{current}"),
    }
}

fn parse_cli(normalized_args: Vec<String>) -> std::result::Result<Cli, i32> {
    let mut parse_input = vec!["multus".to_string()];
    parse_input.extend(normalized_args);

    match Cli::try_parse_from(parse_input) {
        Ok(value) => Ok(value),
        Err(err) => {
            let code = err.exit_code();
            let _ = err.print();
            Err(code)
        }
    }
}

fn execute_command(cli: Cli) -> Result<i32> {
    match cli.command {
        Some(Commands::Split(args)) => handle_split(args),
        Some(Commands::Compress(args)) => handle_compress(args),
        Some(Commands::Merge(args)) => handle_merge(args),
        Some(Commands::Encrypt(args)) => handle_encrypt(args),
        Some(Commands::ImagesToPdf(args)) => handle_images_to_pdf(args),
        Some(Commands::Watermark(args)) => handle_watermark(args),
        Some(Commands::Reorder(args)) => handle_reorder(args),
        Some(Commands::Update(args)) => handle_update(args),
        None => {
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!();
            Ok(1)
        }
    }
}

fn run_interactive() -> i32 {
    ui::prompt::set_interactive_mode(true);
    let items = menu_items();
    let mut version_line = interactive_version_line();

    loop {
        let selected_command = match choose_command_with_arrows(&items, &version_line) {
            Ok(Some(command)) => command,
            Ok(None) => {
                ui::prompt::set_interactive_mode(false);
                println!("Goodbye.");
                return 0;
            }
            Err(err) => {
                ui::prompt::set_interactive_mode(false);
                eprintln!("Error: {err}");
                return 2;
            }
        };
        print_banner();

        let cli = match parse_cli(vec![selected_command]) {
            Ok(value) => value,
            Err(code) => {
                if code != 0 {
                    eprintln!("Error: Failed to parse selected command.");
                }
                continue;
            }
        };

        match execute_command(cli) {
            Ok(_) => {}
            Err(err) if is_back_to_menu_error(&err) => {}
            Err(err) => eprintln!("Error: {err}"),
        }

        version_line = interactive_version_line();
    }
}

fn run(argv: Option<Vec<String>>) -> i32 {
    let args = match argv {
        Some(values) => values,
        None => env::args().skip(1).collect::<Vec<_>>(),
    };

    if args.is_empty() {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            return run_interactive();
        }

        print_banner();
        println!("Non-interactive terminal detected. Running default command: split.");
        let cli = match parse_cli(vec!["split".to_string()]) {
            Ok(value) => value,
            Err(code) => return code,
        };

        return match execute_command(cli) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("Error: {err}");
                2
            }
        };
    }

    let normalized = normalize_argv(args);
    let cli = match parse_cli(normalized) {
        Ok(value) => value,
        Err(code) => return code,
    };

    match execute_command(cli) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Error: {err}");
            2
        }
    }
}

fn main() {
    std::process::exit(run(None));
}

#[cfg(test)]
mod tests {
    use super::has_pdf_extension;
    use crate::cli::normalize_argv;
    use crate::core::path::strip_wrapping_quotes;
    use std::path::Path;

    #[test]
    fn strip_wrapping_quotes_handles_common_inputs() {
        assert_eq!(strip_wrapping_quotes("\"hello\""), "hello");
        assert_eq!(strip_wrapping_quotes("'world'"), "world");
        assert_eq!(strip_wrapping_quotes("  no-quotes  "), "no-quotes");
        assert_eq!(strip_wrapping_quotes("\" spaced \""), "spaced");
    }

    #[test]
    fn has_pdf_extension_is_case_insensitive() {
        assert!(has_pdf_extension(Path::new("file.pdf")));
        assert!(has_pdf_extension(Path::new("file.PDF")));
        assert!(!has_pdf_extension(Path::new("file.docx")));
        assert!(!has_pdf_extension(Path::new("file")));
    }

    #[test]
    fn normalize_argv_defaults_to_split_for_unknown_first_token() {
        let normalized = normalize_argv(vec!["-i".to_string(), "doc.pdf".to_string()]);
        assert_eq!(normalized[0], "split");
    }
}
