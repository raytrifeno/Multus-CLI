use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use std::io::{self, Write};

const MULTUS_ASCII_LOGO_PLAIN: &[&str] = &[
    " __  __       _ _             ",
    "|  \\/  |_   _| | |_ _   _ ___ ",
    "| |\\/| | | | | | __| | | / __|",
    "| |  | | |_| | | |_| |_| \\__ \\",
    "|_|  |_|\\__,_|_|\\__|\\__,_|___/",
];

pub(crate) fn multus_logo_lines() -> &'static [&'static str] {
    MULTUS_ASCII_LOGO_PLAIN
}

pub(crate) fn multus_orange() -> Color {
    Color::Rgb {
        r: 255,
        g: 145,
        b: 0,
    }
}

pub(crate) fn queue_multus_logo<W: Write>(stdout: &mut W) -> io::Result<()> {
    queue!(
        stdout,
        SetForegroundColor(multus_orange()),
        SetAttribute(Attribute::Bold)
    )?;

    for line in multus_logo_lines() {
        queue!(stdout, Print(*line), Print("\n"))?;
    }

    queue!(
        stdout,
        ResetColor,
        SetAttribute(Attribute::Reset),
        Print("\n")
    )?;
    Ok(())
}

pub(crate) fn print_banner() {
    let mut stdout = io::stdout();
    let _ = queue_multus_logo(&mut stdout);
    let _ = stdout.flush();
}

pub(crate) fn print_step(title: &str) {
    println!("\n[{title}]");
}
