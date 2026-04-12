use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "multus",
    version,
    about = "Multus: Split, Compress, Merge, Encrypt, Image Conversion, Watermark, Reorder, Update."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Split(SplitArgs),
    Compress(CompressArgs),
    Merge(MergeArgs),
    Encrypt(EncryptArgs),
    #[command(name = "images-to-pdf", alias = "img2pdf")]
    ImagesToPdf(ImagesToPdfArgs),
    Watermark(WatermarkArgs),
    #[command(alias = "eorder")]
    Reorder(ReorderArgs),
    Update(UpdateArgs),
}

#[derive(Args, Debug, Default, Clone)]
pub struct SplitArgs {
    #[arg(short, long, help = "Path to input file.")]
    pub input: Option<String>,
    #[arg(short, long, help = r#"Page selection, e.g. "1-5,8,10-12"."#)]
    pub pages: Option<String>,
    #[arg(short, long, help = "Output directory.")]
    pub output: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct CompressArgs {
    #[arg(short, long, help = "Path to input file.")]
    pub input: Option<String>,
    #[arg(short, long, help = "Output filename or directory.")]
    pub output: Option<String>,
    #[arg(
        short = 'l',
        long,
        default_value_t = 2,
        value_parser = clap::value_parser!(u8).range(1..=3),
        help = "Compression level: 1 (light), 2 (balanced), 3 (aggressive)."
    )]
    pub level: u8,
}

#[derive(Args, Debug, Default, Clone)]
pub struct MergeArgs {
    #[arg(short = 'i', long = "inputs", num_args = 1.., help = "Paths to input files.")]
    pub inputs: Vec<String>,
    #[arg(short, long, help = "Output filename.")]
    pub output: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct EncryptArgs {
    #[arg(short, long, help = "Path to input file.")]
    pub input: Option<String>,
    #[arg(short, long, help = "User password.")]
    pub password: Option<String>,
    #[arg(
        long = "owner-password",
        help = "Owner password (default: same as user password)."
    )]
    pub owner_password: Option<String>,
    #[arg(short, long, help = "Output filename or directory.")]
    pub output: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct ImagesToPdfArgs {
    #[arg(
        short = 'i',
        long = "inputs",
        num_args = 1..,
        help = "Paths to input image files."
    )]
    pub inputs: Vec<String>,
    #[arg(short, long, help = "Output filename or directory.")]
    pub output: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct WatermarkArgs {
    #[arg(short, long, help = "Path to input file.")]
    pub input: Option<String>,
    #[arg(short, long, help = "Watermark text (example: CONFIDENTIAL).")]
    pub text: Option<String>,
    #[arg(short, long, help = "Output filename or directory.")]
    pub output: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct ReorderArgs {
    #[arg(short, long, help = "Path to input file.")]
    pub input: Option<String>,
    #[arg(
        short,
        long,
        help = r#"New page order, e.g. "10,1-9" (missing pages will be appended)."#
    )]
    pub pages: Option<String>,
    #[arg(short, long, help = "Output filename or directory.")]
    pub output: Option<String>,
}

#[derive(Args, Debug, Default, Clone)]
pub struct UpdateArgs {
    #[arg(long, help = "Repository URL used as update source.")]
    pub repo: Option<String>,
    #[arg(long, help = "Branch or tag to update from.")]
    pub branch: Option<String>,
}

pub fn normalize_argv(mut argv: Vec<String>) -> Vec<String> {
    if argv.is_empty() {
        return argv;
    }

    let first = argv[0].as_str();
    let known = matches!(
        first,
        "split"
            | "compress"
            | "merge"
            | "encrypt"
            | "images-to-pdf"
            | "img2pdf"
            | "watermark"
            | "reorder"
            | "eorder"
            | "update"
            | "help"
            | "-h"
            | "--help"
            | "-V"
            | "--version"
    );
    if !known {
        argv.insert(0, "split".to_string());
    }

    argv
}

pub fn menu_items() -> [(&'static str, &'static str); 8] {
    [
        ("Split pages", "split"),
        ("Compress file size", "compress"),
        ("Merge multiple files", "merge"),
        ("Encrypt file with password", "encrypt"),
        ("Convert images", "images-to-pdf"),
        ("Add watermark", "watermark"),
        ("Reorder pages", "reorder"),
        ("Update Multus", "update"),
    ]
}
