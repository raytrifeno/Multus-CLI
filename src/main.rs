mod cli;
mod commands;
mod core;
mod types;
mod ui;
mod updater;

use clap::{CommandFactory, Parser};
use cli::{Cli, Commands, menu_items, normalize_argv};
use commands::{
    handle_compress, handle_convert_image, handle_encrypt, handle_images_to_pdf, handle_merge,
    handle_reorder, handle_split, handle_uninstall, handle_update, handle_watermark,
};
use std::env;
use std::io::{self, IsTerminal};
use types::Result;
use ui::menu::choose_command_with_arrows;
use updater::{UPDATE_REPO_REF, UPDATE_REPO_URL, VersionState, check_version_state};

pub(crate) use core::image::convert_image_format;
pub(crate) use core::path::{
    build_output_file_path, ensure_output_dir, ensure_output_is_not_input, has_docx_extension,
    has_pdf_extension, has_supported_image_extension, open_pdf, resolve_input_path,
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
            format!("Update available: v{current} -> v{latest} (use Update menu)")
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
        Some(Commands::ConvertImage(args)) => handle_convert_image(args),
        Some(Commands::Watermark(args)) => handle_watermark(args),
        Some(Commands::Reorder(args)) => handle_reorder(args),
        Some(Commands::Update(args)) => handle_update(args),
        Some(Commands::Uninstall(args)) => handle_uninstall(args),
        None => {
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!();
            Ok(1)
        }
    }
}

fn should_skip_update_notice(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(Commands::Update(_)) | Some(Commands::Uninstall(_)) | None
    )
}

fn print_update_notice_if_available() {
    if let VersionState::UpdateAvailable { current, latest } =
        check_version_state(UPDATE_REPO_URL, UPDATE_REPO_REF)
    {
        eprintln!(
            "\nUpdate available: v{current} -> v{latest}. Run `multus update` to install it."
        );
    }
}

fn execute_command_with_notice(cli: Cli) -> Result<i32> {
    let skip_notice = should_skip_update_notice(&cli);
    let result = execute_command(cli);
    if !skip_notice {
        print_update_notice_if_available();
    }
    result
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

        return match execute_command_with_notice(cli) {
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

    match execute_command_with_notice(cli) {
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
