use crossterm::queue;
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use std::io::{self, Write};

const BRAND_NAME: &str = "MULTUS";
const BRAND_TAGLINE: &str = "Document toolkit";

pub(crate) fn brand_name() -> &'static str {
    BRAND_NAME
}

pub(crate) fn brand_tagline() -> &'static str {
    BRAND_TAGLINE
}

pub(crate) fn multus_orange() -> Color {
    Color::Rgb {
        r: 255,
        g: 145,
        b: 0,
    }
}

pub(crate) fn queue_brand_header<W: Write>(stdout: &mut W) -> io::Result<()> {
    queue!(
        stdout,
        SetForegroundColor(multus_orange()),
        SetAttribute(Attribute::Bold),
        Print(BRAND_NAME),
        ResetColor,
        SetAttribute(Attribute::Reset),
        Print("\n"),
        Print(BRAND_TAGLINE),
        Print("\n\n")
    )?;
    Ok(())
}

pub(crate) fn print_banner() {
    let mut stdout = io::stdout();
    let _ = queue_brand_header(&mut stdout);
    let _ = stdout.flush();
}

pub(crate) fn print_step(title: &str) {
    println!("\n[{title}]");
}
