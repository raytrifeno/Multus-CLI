# Multus (Rust CLI)

Multus is a Rust CLI tool for document workflows, designed to run directly in terminal.

## Key Features

- Split documents by page ranges.
- Compress documents for smaller file size.
- Merge multiple documents into one file.
- Encrypt files with a password.
- Convert images into PDF.
- Convert image format to JPG or PNG.
- Add text watermark to `.pdf` and `.docx`.
- Reorder pages with custom order.
- Update the tool using `multus update`.

## Install

Source repository:

`https://github.com/raytrifeno/Multus-CLI.git`

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/raytrifeno/Multus-CLI/main/scripts/install.ps1 -UseBasicParsing | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/raytrifeno/Multus-CLI/main/scripts/install.sh | bash
```

Installer behavior:

- Downloads a prebuilt binary from the latest GitHub Release.
- Does not require Rust or Cargo on end-user machines.
- Installs executable command: `multus`.

Supported release assets:

- `multus-windows-x64.zip`
- `multus-linux-x64.tar.gz`
- `multus-macos-x64.tar.gz`
- `multus-macos-arm64.tar.gz`

## Uninstall

### Windows (PowerShell)

```powershell
iwr https://raw.githubusercontent.com/raytrifeno/Multus-CLI/main/scripts/uninstall.ps1 -UseBasicParsing | iex
```

### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/raytrifeno/Multus-CLI/main/scripts/uninstall.sh | bash
```

## Build

```powershell
cargo build --release
```

## Release Automation

GitHub Actions builds and publishes release binaries on:

- tag push (`v*`)
- manual workflow dispatch

## Usage

### Interactive Mode

```powershell
multus
```

### Split

```powershell
multus split -i document.pdf -p "1-5,8,10-12" -o .\output\
```

### Compress

```powershell
multus compress -i document.pdf -l 2 -o compressed.pdf
```

Level:

- `1` = light
- `2` = balanced (default)
- `3` = aggressive

### Merge

```powershell
multus merge -i file1.pdf file2.pdf file3.pdf -o merged.pdf
```

### Encrypt

```powershell
multus encrypt -i document.pdf -p "mypassword" -o encrypted.pdf
```

### Convert Images to PDF

```powershell
multus images-to-pdf -i img1.png img2.jpg -o output.pdf
```

Alias:

```powershell
multus img2pdf -i img1.png img2.jpg -o output.pdf
```

### Convert Image Format

```powershell
multus convert-image -i img1.png img2.bmp -f jpg -o .\converted\
```

Alias:

```powershell
multus imgconvert -i img1.png -f png -o output.png
```

### Watermark

```powershell
multus watermark -i document.pdf -t "CONFIDENTIAL" -o watermarked.pdf
```

Also supports `.docx` input.

### Reorder Pages

```powershell
multus reorder -i document.pdf -p "10,1-9" -o reordered.pdf
```

Alias:

```powershell
multus eorder -i document.pdf -p "10,1-9" -o reordered.pdf
```

## Page Selection Format

Use:

`1-5,8,10-12`

## Tests

```powershell
cargo test
```
