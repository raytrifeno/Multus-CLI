# Multus (Rust CLI)

<pre><span style="color:#ff9100">
███╗   ███╗██╗   ██╗██╗  ████████╗██╗   ██╗███████╗
████╗ ████║██║   ██║██║  ╚══██╔══╝██║   ██║██╔════╝
██╔████╔██║██║   ██║██║     ██║   ██║   ██║███████╗
██║╚██╔╝██║██║   ██║██║     ██║   ██║   ██║╚════██║
██║ ╚═╝ ██║╚██████╔╝███████╗██║   ╚██████╔╝███████║
╚═╝     ╚═╝ ╚═════╝ ╚══════╝╚═╝    ╚═════╝ ╚══════╝
</span></pre>

Multus is a Rust-based CLI tool for document workflows directly from the terminal: fast, lightweight, and without web services.

## Key Features

- Split documents by page ranges.
- Compress documents for smaller file size.
- Merge multiple documents into one file.
- Encrypt files with a password.
- Convert images into document output.
- Add text watermark to supported files.
- Reorder pages with custom order.
- Update the tool with the `multus update` command.

## Install

Source repository:

`https://github.com/raytrifeno/scraks.git`

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/raytrifeno/scraks/main/scripts/install.ps1 -UseBasicParsing | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/raytrifeno/scraks/main/scripts/install.sh | bash
```