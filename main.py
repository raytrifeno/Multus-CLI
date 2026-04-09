from __future__ import annotations

import argparse
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Sequence, Tuple

try:
    from pypdf import PdfReader, PdfWriter
except Exception as exc:  # pragma: no cover - handled at runtime
    raise SystemExit(
        "Missing dependency 'pypdf'. Install with: pip install -r requirements.txt"
    ) from exc

try:
    import fitz
except ImportError:
    # We will handle the error when the compress command is actually used
    fitz = None


class PageSelectionError(ValueError):
    pass


@dataclass(frozen=True)
class ParsedSelection:
    pages: List[int]
    groups: List[List[int]]
    raw: str


def parse_page_selection(selection: str) -> ParsedSelection:
    """
    Parse a page selection string like "1-5,8,10-12".
    Returns:
      - pages: sorted unique pages (1-based)
      - groups: list of page groups per token (ranges stay merged)
    """
    if selection is None:
        raise PageSelectionError("Page selection is required.")

    raw = selection.strip()
    if not raw:
        raise PageSelectionError("Page selection is empty.")

    pages: set[int] = set()
    groups: List[List[int]] = []
    parts = [p.strip() for p in raw.split(",") if p.strip()]
    if not parts:
        raise PageSelectionError("Page selection is invalid.")

    for part in parts:
        if "-" in part:
            bounds = [b.strip() for b in part.split("-")]
            if len(bounds) != 2 or not bounds[0] or not bounds[1]:
                raise PageSelectionError(f"Invalid range: '{part}'")
            try:
                start = int(bounds[0])
                end = int(bounds[1])
            except ValueError as exc:
                raise PageSelectionError(f"Invalid range numbers: '{part}'") from exc
            if start < 1 or end < 1:
                raise PageSelectionError(f"Range must be >= 1: '{part}'")
            if start > end:
                raise PageSelectionError(f"Range start > end: '{part}'")
            group = list(range(start, end + 1))
            groups.append(group)
            pages.update(group)
        else:
            try:
                page = int(part)
            except ValueError as exc:
                raise PageSelectionError(f"Invalid page number: '{part}'") from exc
            if page < 1:
                raise PageSelectionError(f"Page must be >= 1: '{part}'")
            pages.add(page)
            groups.append([page])

    return ParsedSelection(pages=sorted(pages), groups=groups, raw=raw)


def validate_pages(pages: Sequence[int], total_pages: int) -> List[int]:
    if total_pages < 1:
        raise PageSelectionError("PDF has no pages.")
    out_of_range = [p for p in pages if p < 1 or p > total_pages]
    if out_of_range:
        raise PageSelectionError(
            f"Pages out of range (1-{total_pages}): {', '.join(map(str, out_of_range))}"
        )
    return list(pages)


def _strip_wrapping_quotes(value: str) -> str:
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in ("\"", "'"):
        return value[1:-1].strip()
    return value


def ensure_output_dir(output_arg: str | None) -> Path:
    cleaned = _strip_wrapping_quotes(output_arg) if output_arg else ""
    output_dir = Path(cleaned) if cleaned else Path.cwd()
    if not output_dir.is_absolute():
        output_dir = (Path.cwd() / output_dir).resolve()
    else:
        output_dir = output_dir.resolve()

    output_dir.mkdir(parents=True, exist_ok=True)
    return output_dir


def resolve_input_path(input_arg: str) -> Path:
    cleaned = _strip_wrapping_quotes(input_arg)
    input_path = Path(cleaned).expanduser()
    if not input_path.is_absolute():
        input_path = (Path.cwd() / input_path).resolve()
    return input_path


def open_pdf(input_path: Path) -> Tuple[PdfReader, int]:
    if input_path.suffix.lower() != ".pdf":
        raise PageSelectionError(f"Input is not a PDF file: '{input_path}'")
    try:
        f = input_path.open("rb")
    except FileNotFoundError as exc:
        raise PageSelectionError(f"File not found: '{input_path}'") from exc
    except IsADirectoryError as exc:
        raise PageSelectionError(f"Input path is a directory: '{input_path}'") from exc
    except Exception as exc:
        raise PageSelectionError(f"Failed to open file: '{input_path}'") from exc

    try:
        reader = PdfReader(f)
        total_pages = len(reader.pages)
    except Exception as exc:
        f.close()
        raise PageSelectionError(f"Failed to read PDF: '{input_path}'") from exc

    return reader, total_pages


def split_pdf(
    input_path: Path,
    reader: PdfReader,
    groups: Iterable[Iterable[int]],
    output_dir: Path,
) -> int:
    count = 0
    for group in groups:
        pages = list(group)
        if not pages:
            continue
        writer = PdfWriter()
        for page_num in pages:
            writer.add_page(reader.pages[page_num - 1])

        if len(pages) == 1:
            label = f"{pages[0]}"
        else:
            label = f"{pages[0]}-{pages[-1]}"
        output_name = f"{input_path.stem}_halaman_{label}.pdf"
        output_path = output_dir / output_name
        with output_path.open("wb") as out_f:
            writer.write(out_f)
        count += 1

    return count


def compress_pdf(input_path: Path, output_path: Path) -> int:
    if fitz is None:
        raise PageSelectionError("Missing dependency 'pymupdf'. Install with: pip install pymupdf")
    
    try:
        with fitz.open(str(input_path)) as doc:
            doc.save(
                str(output_path),
                garbage=3,
                deflate=True,
                use_objstms=True
            )
        return 1
    except Exception as exc:
        raise PageSelectionError(f"Failed to compress PDF: {exc}")


def merge_pdfs(input_paths: List[Path], output_path: Path) -> int:
    writer = PdfWriter()
    for path in input_paths:
        try:
            reader = PdfReader(str(path))
            for page in reader.pages:
                writer.add_page(page)
        except Exception as exc:
            raise PageSelectionError(f"Failed to read file for merging: '{path}'. {exc}")
    
    try:
        with output_path.open("wb") as out_f:
            writer.write(out_f)
        return 1
    except Exception as exc:
        raise PageSelectionError(f"Failed to save merged PDF: {exc}")


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="PDF tools: Split, Compress, and Merge."
    )
    subparsers = parser.add_subparsers(dest="command", help="Command to run")

    # Split command
    split_parser = subparsers.add_parser("split", help="Split a PDF into per-range PDFs.")
    split_parser.add_argument("-i", "--input", help="Path to input PDF.")
    split_parser.add_argument("-p", "--pages", help='Page selection, e.g. "1-5,8,10-12".')
    split_parser.add_argument("-o", "--output", help="Output directory.")

    # Compress command
    compress_parser = subparsers.add_parser("compress", help="Compress a PDF file.")
    compress_parser.add_argument("-i", "--input", help="Path to input PDF.")
    compress_parser.add_argument("-o", "--output", help="Output filename or directory.")

    # Merge command
    merge_parser = subparsers.add_parser("merge", help="Merge multiple PDFs into one.")
    merge_parser.add_argument("-i", "--inputs", nargs="+", help="Paths to input PDFs.")
    merge_parser.add_argument("-o", "--output", help="Output filename.")

    return parser


def prompt_non_empty(prompt: str) -> str:
    while True:
        value = input(prompt).strip()
        if value:
            return value
        print("Input tidak boleh kosong.")


def prompt_optional(prompt: str) -> str:
    return input(prompt).strip()


def print_banner() -> None:
    # ANSI colors (works in most modern terminals)
    cyan = "\x1b[36m"
    bright = "\x1b[1m"
    dim = "\x1b[2m"
    reset = "\x1b[0m"
    banner = r"""
   ________          __           
  / ____/ /__  _  __/ /____  _____
 / /   / / _ \| |/_/ __/ _ \/ ___/
/ /___/ /  __/>  </ /_/  __/ /    
\____/_/\___/_/|_|\__/\___/_/     
"""
    print(f"{cyan}{bright}{banner}{reset}")
    print(f"{bright}CODEx Terminal // PDF Tools{reset}")
    print(f"{dim}{'=' * 44}{reset}")


def print_step(title: str) -> None:
    print(f"\n[{title}]")


def run_with_spinner(message: str, func):
    stop_event = threading.Event()

    def spin():
        frames = "|/-\\"
        i = 0
        while not stop_event.is_set():
            sys.stdout.write(f"\r{message} {frames[i % len(frames)]}")
            sys.stdout.flush()
            i += 1
            time.sleep(0.08)
        sys.stdout.write("\r" + " " * (len(message) + 2) + "\r")
        sys.stdout.flush()

    t = threading.Thread(target=spin, daemon=True)
    t.start()
    try:
        return func()
    finally:
        stop_event.set()
        t.join()


def handle_split(args, argv: Sequence[str] | None = None) -> int:
    if args.input:
        input_value = args.input
    else:
        print_step("INPUT PDF")
        input_value = prompt_non_empty("Masukkan path file PDF: ")

    input_path = resolve_input_path(input_value)
    reader, total_pages = run_with_spinner(
        "Memverifikasi PDF...", lambda: open_pdf(input_path)
    )

    if args.pages:
        pages_value = args.pages
    else:
        print_step("PILIH HALAMAN")
        pages_value = prompt_non_empty(
            'Masukkan range halaman (contoh "1-5,8,10-12"): '
        )

    selection = parse_page_selection(pages_value)
    validate_pages(selection.pages, total_pages)

    if args.output:
        output_value = args.output
    else:
        print_step("OUTPUT")
        output_value = prompt_optional(
            "Simpan di folder mana? (kosong = folder saat ini): "
        )
        if not output_value:
            output_value = str(Path.cwd())

    output_dir = ensure_output_dir(output_value)
    count = split_pdf(input_path, reader, selection.groups, output_dir)
    print(f"Saved {count} file(s) to: {output_dir}")
    return 0


def handle_compress(args) -> int:
    if args.input:
        input_value = args.input
    else:
        print_step("INPUT PDF")
        input_value = prompt_non_empty("Masukkan path file PDF: ")

    input_path = resolve_input_path(input_value)
    if not input_path.exists():
        raise PageSelectionError(f"File not found: '{input_path}'")
    if input_path.suffix.lower() != ".pdf":
        raise PageSelectionError(f"Input is not a PDF file: '{input_path}'")

    if args.output:
        output_value = args.output
    else:
        print_step("OUTPUT")
        output_value = prompt_optional(
            f"Simpan sebagai apa? (kosong = {input_path.stem}_compressed.pdf): "
        )
        if not output_value:
            output_value = str(input_path.parent / f"{input_path.stem}_compressed.pdf")

    output_path = Path(_strip_wrapping_quotes(output_value))
    if not output_path.is_absolute():
        output_path = (Path.cwd() / output_path).resolve()
    
    if output_path.is_dir():
        output_path = output_path / f"{input_path.stem}_compressed.pdf"
    elif not output_path.suffix:
        output_path.mkdir(parents=True, exist_ok=True)
        output_path = output_path / f"{input_path.stem}_compressed.pdf"

    output_path.parent.mkdir(parents=True, exist_ok=True)

    run_with_spinner(
        "Sedang mengompresi PDF...", 
        lambda: compress_pdf(input_path, output_path)
    )
    
    original_size = input_path.stat().st_size
    compressed_size = output_path.stat().st_size
    reduction = (1 - (compressed_size / original_size)) * 100
    
    print(f"Compression complete!")
    print(f"Original size:   {original_size / 1024 / 1024:.2f} MB")
    print(f"Compressed size: {compressed_size / 1024 / 1024:.2f} MB")
    print(f"Reduction:       {reduction:.2f}%")
    print(f"Saved to: {output_path}")
    return 0


def handle_merge(args) -> int:
    if args.inputs:
        input_values = args.inputs
    else:
        print_step("INPUT PDFS")
        raw = prompt_non_empty("Masukkan path file-file PDF (pisahkan dengan spasi atau koma): ")
        if "," in raw:
            input_values = [x.strip() for x in raw.split(",") if x.strip()]
        else:
            input_values = [x.strip() for x in raw.split() if x.strip()]

    input_paths = [resolve_input_path(val) for val in input_values]
    for p in input_paths:
        if not p.exists():
            raise PageSelectionError(f"File not found: '{p}'")

    if args.output:
        output_value = args.output
    else:
        print_step("OUTPUT")
        output_value = prompt_non_empty("Simpan sebagai apa? (contoh: merged.pdf): ")

    output_path = Path(_strip_wrapping_quotes(output_value))
    if not output_path.is_absolute():
        output_path = (Path.cwd() / output_path).resolve()
    
    output_path.parent.mkdir(parents=True, exist_ok=True)

    run_with_spinner(
        "Sedang menggabungkan PDF...", 
        lambda: merge_pdfs(input_paths, output_path)
    )
    
    print(f"Merge complete!")
    print(f"Saved to: {output_path}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    modified_argv = list(argv) if argv is not None else sys.argv[1:]
    
    if not modified_argv:
        print_banner()
        print("Pilih tool:")
        print("1. Split PDF (pecah halaman)")
        print("2. Compress PDF (kecilkan ukuran)")
        print("3. Merge PDF (gabung file)")
        choice = input("\nPilihan (1/2/3): ").strip()
        if choice == "2":
            modified_argv = ["compress"]
        elif choice == "3":
            modified_argv = ["merge"]
        else:
            modified_argv = ["split"]
    elif modified_argv[0] not in ["split", "compress", "merge", "-h", "--help"]:
        modified_argv.insert(0, "split")

    parser = build_arg_parser()
    args = parser.parse_args(modified_argv)

    try:
        if args.command == "split":
            return handle_split(args, argv)
        elif args.command == "compress":
            return handle_compress(args)
        elif args.command == "merge":
            return handle_merge(args)
        else:
            parser.print_help()
            return 1
    except PageSelectionError as exc:
        print(f"Error: {exc}", file=sys.stderr)
        return 2
    except Exception as exc:
        print(f"Unexpected error: {exc}", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main())
