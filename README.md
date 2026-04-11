# Multus (Rust CLI)

Multus is a pure Rust CLI toolkit for PDF/document workflows. **All web features have been removed**.

Available features:

- Split PDF by page ranges.
- Compress PDF.
- Merge multiple PDFs.
- Encrypt PDF (password protect).
- Convert images to PDF.
- Add text watermark to PDF and DOCX.
- Reorder PDF pages.

## Install (recommended)

Repository source:

`https://github.com/raytrifeno/scraks.git`

### Windows (PowerShell + iwr)

```powershell
iwr https://raw.githubusercontent.com/raytrifeno/scraks/main/scripts/install.ps1 -UseBasicParsing | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/raytrifeno/scraks/main/scripts/install.sh | bash
```

Installer behavior:

- Requires Rust/Cargo to already be installed.
- Uses a **Parallel Task Runner UI**:
  - up to 10 packages visible
  - up to 3 active tasks in parallel
  - finished tasks disappear and are replaced by queued tasks
- Installs the CLI command as: `multus`.

Rust prerequisite (manual install):

`https://www.rust-lang.org/tools/install`

## Uninstall

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/raytrifeno/scraks/main/scripts/uninstall.ps1 -UseBasicParsing | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/raytrifeno/scraks/main/scripts/uninstall.sh | bash
```

Uninstall behavior:

- Removes `multus` binary via `cargo uninstall multus` (if cargo is available).
- Prompts whether to remove downloaded Cargo package cache.
- Prompts whether to also remove Rust installation (`~/.rustup` and `~/.cargo`).

## Build from source

```powershell
cargo build --release
```

## Usage

Run without arguments (interactive mode):

```powershell
multus
```

or from source:

```powershell
cargo run
```

Interactive mode:

- Use arrow keys (`↑` / `↓`) to move.
- Selected option is highlighted in orange.
- Press `Enter` to choose.
- Press `Q` twice in menu to exit.
- Type `QQ` in any interactive prompt to return to menu.

Split:

```powershell
multus split -i "C:\data\example.pdf" -p "1-5,8,10-12"
```

Compress:

```powershell
multus compress -i "C:\data\example.pdf" -l 2 -o "C:\data\compressed.pdf"
```

Compression levels:

- `-l 1` = light (higher quality, smaller reduction)
- `-l 2` = balanced (default)
- `-l 3` = aggressive (smaller files, more image quality loss)

Note: if compressed output is larger, Multus automatically keeps the original size.

Merge:

```powershell
multus merge -i "C:\data\a.pdf" "C:\data\b.pdf" -o "C:\data\merged.pdf"
```

Encrypt:

```powershell
multus encrypt -i "C:\data\example.pdf" -p "password123" -o "C:\data\example_encrypted.pdf"
```

Images to PDF:

```powershell
multus images-to-pdf -i "C:\img\1.jpg" "C:\img\2.png" -o "C:\img\result.pdf"
```

Watermark:

```powershell
multus watermark -i "C:\data\example.pdf" -t "CONFIDENTIAL" -o "C:\data\example_watermarked.pdf"
```

```powershell
multus watermark -i "C:\data\document.docx" -t "CONFIDENTIAL" -o "C:\data\document_watermarked.docx"
```

Reorder:

```powershell
multus reorder -i "C:\data\example.pdf" -p "10,1-9" -o "C:\data\example_reordered.pdf"
```

If you pass `-i/-p/-o` directly without a subcommand, Multus automatically runs `split` mode (same behavior as before).

## Page Format

Use this format:

`1-5,8,10-12`

## Unit Tests

```powershell
cargo test
```

## Output Naming Example

If input is `report.pdf` and page is `3`:

`report_page_3.pdf`

If input is `report.pdf` and range is `1-3,8`:

`report_page_1-3.pdf` and `report_page_8.pdf`
