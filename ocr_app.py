from __future__ import annotations

import io
import logging
import os
import shutil
import sysconfig
from pathlib import Path
from typing import Any, Dict, List, Tuple

import fitz
import pytesseract
from fastapi import FastAPI, File, Form, HTTPException, UploadFile, Response
from fastapi.staticfiles import StaticFiles
from PIL import Image, UnidentifiedImageError

APP_TITLE = "PDF OCR Studio"
MAX_UPLOAD_MB = 200
MAX_UPLOAD_BYTES = MAX_UPLOAD_MB * 1024 * 1024
SUPPORTED_IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".bmp", ".tif", ".tiff", ".webp"}

app = FastAPI(title=APP_TITLE, version="1.0.0")
logger = logging.getLogger("uvicorn.error")


class OCRProcessingError(RuntimeError):
    pass


def _resolve_tesseract_cmd() -> str | None:
    env_cmd = os.getenv("TESSERACT_CMD", "").strip()
    if env_cmd and Path(env_cmd).exists():
        return env_cmd

    which_cmd = shutil.which("tesseract")
    if which_cmd:
        return which_cmd

    windows_candidates = [
        Path(r"C:\Program Files\Tesseract-OCR\tesseract.exe"),
        Path(r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe"),
        Path.home() / "AppData/Local/Programs/Tesseract-OCR/tesseract.exe",
    ]
    for candidate in windows_candidates:
        if candidate.exists():
            return str(candidate)

    return None


def _ensure_tesseract_ready() -> None:
    tesseract_cmd = _resolve_tesseract_cmd()
    if tesseract_cmd:
        pytesseract.pytesseract.tesseract_cmd = tesseract_cmd

    try:
        pytesseract.get_tesseract_version()
    except Exception as exc:
        raise OCRProcessingError(
            "Tesseract is not available. Install with 'winget install UB-Mannheim.TesseractOCR', "
            "restart terminal, or set TESSERACT_CMD to full path of tesseract.exe."
        ) from exc


def _normalize_lang(lang: str | None) -> str:
    value = (lang or "").strip()
    if not value:
        return "eng"
    return value


def _resolve_ocr_lang(requested_lang: str) -> Tuple[str, List[str]]:
    raw = requested_lang.replace(",", "+")
    requested = [x.strip() for x in raw.split("+") if x.strip()]
    if not requested:
        requested = ["eng"]

    try:
        available = set(pytesseract.get_languages(config=""))
    except Exception as exc:
        raise OCRProcessingError(
            "Failed to read available OCR languages from Tesseract."
        ) from exc

    valid: List[str] = []
    missing: List[str] = []
    seen = set()
    for code in requested:
        if code in seen:
            continue
        seen.add(code)
        if code in available:
            valid.append(code)
        else:
            missing.append(code)

    if not valid:
        if "eng" in available:
            valid = ["eng"]
        else:
            available_text = ", ".join(sorted(available)) or "none"
            raise OCRProcessingError(
                f"Requested OCR language is unavailable. Available: {available_text}"
            )

    return "+".join(valid), missing


def _ocr_image_bytes(image_bytes: bytes, lang: str) -> str:
    try:
        with Image.open(io.BytesIO(image_bytes)) as image:
            return pytesseract.image_to_string(image, lang=lang).strip()
    except UnidentifiedImageError as exc:
        raise OCRProcessingError("Uploaded file is not a valid image.") from exc
    except Exception as exc:
        raise OCRProcessingError(f"OCR failed for image: {exc}") from exc


def _ocr_pdf_bytes(pdf_bytes: bytes, lang: str) -> List[Dict[str, Any]]:
    pages: List[Dict[str, Any]] = []
    try:
        with fitz.open(stream=pdf_bytes, filetype="pdf") as document:
            if document.page_count < 1:
                raise OCRProcessingError("PDF has no pages.")

            for page_index in range(document.page_count):
                page = document.load_page(page_index)
                pixmap = page.get_pixmap(dpi=220, alpha=False)
                image_bytes = pixmap.tobytes("png")
                text = _ocr_image_bytes(image_bytes, lang)
                pages.append(
                    {
                        "page": page_index + 1,
                        "text": text,
                        "char_count": len(text),
                    }
                )
    except OCRProcessingError:
        raise
    except Exception as exc:
        raise OCRProcessingError("Failed to read PDF for OCR.") from exc

    return pages


def _compress_pdf_bytes(pdf_bytes: bytes) -> bytes:
    try:
        with fitz.open(stream=pdf_bytes, filetype="pdf") as doc:
            output_stream = io.BytesIO()
            # garbage=3: remove unused objects, compact xref table, merge duplicate objects
            # deflate=True: compress uncompressed streams
            # use_objstms=True: pack objects into object streams
            doc.save(
                output_stream,
                garbage=3,
                deflate=True,
                use_objstms=True,
            )
            return output_stream.getvalue()
    except Exception as exc:
        raise OCRProcessingError(f"Failed to compress PDF: {exc}") from exc


def _detect_file_kind(filename: str | None, content: bytes) -> str:
    suffix = Path(filename or "").suffix.lower()
    if suffix == ".pdf" or content.startswith(b"%PDF-"):
        return "pdf"
    if suffix in SUPPORTED_IMAGE_EXTENSIONS:
        return "image"
    try:
        with Image.open(io.BytesIO(content)):
            return "image"
    except Exception:
        pass
    raise OCRProcessingError("Only PDF and image files are supported.")


@app.get("/api/health")
def health() -> Dict[str, str]:
    return {"status": "ok"}


@app.post("/api/ocr")
async def ocr_file(
    file: UploadFile = File(...),
    lang: str = Form("eng"),
) -> Dict[str, Any]:
    logger.info("OCR request started: filename=%s lang=%s", file.filename, lang)
    content = await file.read()
    if not content:
        raise HTTPException(status_code=400, detail="File is empty.")
    if len(content) > MAX_UPLOAD_BYTES:
        raise HTTPException(
            status_code=413,
            detail=f"File is too large. Maximum size is {MAX_UPLOAD_MB} MB.",
        )

    parsed_lang = _normalize_lang(lang)
    try:
        _ensure_tesseract_ready()
        effective_lang, missing_langs = _resolve_ocr_lang(parsed_lang)
        kind = _detect_file_kind(file.filename, content)
        if kind == "pdf":
            pages = _ocr_pdf_bytes(content, effective_lang)
        else:
            text = _ocr_image_bytes(content, effective_lang)
            pages = [{"page": 1, "text": text, "char_count": len(text)}]
    except OCRProcessingError as exc:
        logger.warning("OCR request failed for '%s': %s", file.filename, exc)
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    merged_text = "\n\n".join(page["text"] for page in pages if page["text"])
    return {
        "filename": file.filename or "uploaded_file",
        "kind": kind,
        "lang": effective_lang,
        "requested_lang": parsed_lang,
        "missing_langs": missing_langs,
        "page_count": len(pages),
        "char_count": len(merged_text),
        "pages": pages,
    }


@app.post("/api/compress")
async def compress_file(file: UploadFile = File(...)) -> Response:
    logger.info("Compression request started: filename=%s", file.filename)
    content = await file.read()
    if not content:
        raise HTTPException(status_code=400, detail="File is empty.")
    if len(content) > MAX_UPLOAD_BYTES:
        raise HTTPException(
            status_code=413,
            detail=f"File is too large. Maximum size is {MAX_UPLOAD_MB} MB.",
        )

    try:
        kind = _detect_file_kind(file.filename, content)
        if kind != "pdf":
            raise OCRProcessingError("Compression is only supported for PDF files.")

        compressed_bytes = _compress_pdf_bytes(content)
        original_size = len(content)
        compressed_size = len(compressed_bytes)
        reduction = (1 - (compressed_size / original_size)) * 100

        logger.info(
            "PDF compressed: original=%d bytes, compressed=%d bytes, reduction=%.2f%%",
            original_size,
            compressed_size,
            reduction,
        )

        filename = file.filename or "compressed.pdf"
        if not filename.lower().endswith(".pdf"):
            filename += ".pdf"
        if not filename.lower().endswith(".pdf"):
            filename = Path(filename).stem + "_compressed.pdf"
        else:
            filename = filename.replace(".pdf", "_compressed.pdf")

        return Response(
            content=compressed_bytes,
            media_type="application/pdf",
            headers={"Content-Disposition": f"attachment; filename={filename}"},
        )
    except OCRProcessingError as exc:
        logger.warning("Compression request failed for '%s': %s", file.filename, exc)
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except Exception as exc:
        logger.error("Internal error during compression: %s", exc)
        raise HTTPException(status_code=500, detail="Internal server error.") from exc


def resolve_web_dir() -> Path:
    local_web = Path(__file__).resolve().parent / "web"
    if local_web.exists():
        return local_web

    installed_web = Path(sysconfig.get_paths()["purelib"]) / "web"
    if installed_web.exists():
        return installed_web

    raise RuntimeError("Web assets not found. Reinstall package to include web files.")


WEB_DIR = resolve_web_dir()
app.mount("/", StaticFiles(directory=WEB_DIR, html=True), name="web")


def run() -> None:
    import uvicorn

    uvicorn.run("ocr_app:app", host="127.0.0.1", port=8787, reload=False)


if __name__ == "__main__":
    run()
