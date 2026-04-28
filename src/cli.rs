use clap::{Args, Parser, Subcommand};

pub type MenuItem = (&'static str, &'static str, &'static str);

#[derive(Parser, Debug)]
#[command(
    name = "multus",
    version,
    about = "Multus: Split, Compress, Merge, Encrypt, Image Conversion, Image Format Conversion, Watermark, Reorder, Update, Uninstall."
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
    #[command(name = "convert-image", aliases = ["imgconvert", "imgext"])]
    ConvertImage(ConvertImageArgs),
    Watermark(WatermarkArgs),
    #[command(alias = "eorder")]
    Reorder(ReorderArgs),
    Update(UpdateArgs),
    Uninstall(UninstallArgs),
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
pub struct ConvertImageArgs {
    #[arg(
        short = 'i',
        long = "inputs",
        num_args = 1..,
        help = "Paths to input image files."
    )]
    pub inputs: Vec<String>,
    #[arg(
        short = 'f',
        long = "format",
        value_parser = ["jpg", "jpeg", "png"],
        help = "Target output format: jpg or png."
    )]
    pub format: Option<String>,
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

#[derive(Args, Debug, Default, Clone)]
pub struct UninstallArgs {
    #[arg(short, long, help = "Run uninstall without asking for confirmation.")]
    pub yes: bool,
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
            | "convert-image"
            | "imgconvert"
            | "imgext"
            | "watermark"
            | "reorder"
            | "eorder"
            | "update"
            | "uninstall"
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

pub fn menu_items() -> [MenuItem; 10] {
    [
        ("Split pages", "Select pages from one PDF", "split"),
        (
            "Compress file size",
            "Reduce PDF size with quality profiles",
            "compress",
        ),
        (
            "Merge multiple files",
            "Combine many PDFs into one output",
            "merge",
        ),
        (
            "Encrypt file with password",
            "Protect a PDF with user and owner passwords",
            "encrypt",
        ),
        (
            "Convert images to PDF",
            "Build a PDF from one or many images",
            "images-to-pdf",
        ),
        (
            "Convert image format",
            "Change images to JPG or PNG",
            "convert-image",
        ),
        (
            "Add watermark",
            "Stamp text on PDF or DOCX pages",
            "watermark",
        ),
        (
            "Reorder pages",
            "Rebuild page order without editing the source",
            "reorder",
        ),
        (
            "Update Multus",
            "Download and install the latest release build",
            "update",
        ),
        (
            "Uninstall Multus",
            "Remove Multus from this machine",
            "uninstall",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::normalize_argv;

    #[test]
    fn normalize_argv_defaults_to_split_for_unknown_first_token() {
        let normalized = normalize_argv(vec!["-i".to_string(), "doc.pdf".to_string()]);
        assert_eq!(normalized[0], "split");
    }

    #[test]
    fn normalize_argv_keeps_uninstall_subcommand() {
        let normalized = normalize_argv(vec!["uninstall".to_string(), "--yes".to_string()]);
        assert_eq!(normalized[0], "uninstall");
    }
}
