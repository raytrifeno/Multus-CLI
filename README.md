# PDF Split Tool + OCR UI

Toolkit ini punya dua fitur:

- Split PDF per range via CLI.
- OCR modern berbasis web UI drag-and-drop.

## Setup

Aktifkan venv dan install dependency:

```powershell
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt
```

## Prasyarat OCR (Tesseract)

Fitur OCR memakai `pytesseract` dengan engine Tesseract (native C/C++).

Windows (winget):

```powershell
winget install UB-Mannheim.TesseractOCR
```

Setelah install, buka terminal baru agar PATH ter-refresh.

Verifikasi:

```powershell
tesseract --version
```

Jika command belum dikenali, set manual:

```powershell
$env:TESSERACT_CMD="C:\Program Files\Tesseract-OCR\tesseract.exe"
```

## Install CLI (pipx/pip)

Jika ingin menjalankan dari mana saja dengan command `pdf`:

```powershell
pipx install -e C:\Users\raysu\Downloads\split
```

Atau dengan pip (butuh aktivasi venv dulu):

```powershell
.\.venv\Scripts\Activate.ps1
pip install -e .
```

Setelah itu, jalankan CLI split:

```powershell
pdf
```

Untuk OCR UI:

```powershell
pdf-ocr-ui
```

Lalu buka browser ke `http://127.0.0.1:8787`.

## Pemakaian

```powershell
python main.py -i "C:\data\contoh.pdf" -p "1-5,8,10-12"
```

Output default ada di folder saat ini. Bisa override dengan `-o`:

```powershell
python main.py -i "..\file.pdf" -p "2,4-6" -o "D:\output\hasil"
```

Jika dijalankan tanpa argumen, program akan meminta input secara interaktif.

## OCR Drag & Drop UI

Menjalankan tanpa install global:

```powershell
.\.venv\Scripts\Activate.ps1
uvicorn ocr_app:app --host 127.0.0.1 --port 8787 --reload
```

Fitur UI:

- Drag and drop file PDF/gambar.
- OCR per halaman untuk PDF.
- Copy hasil per halaman atau copy semua.
- Input bahasa OCR (default `eng`; bisa `eng+ind` jika language pack tersedia).

## Format Halaman

Gunakan format `1-5,8,10-12`.

## Error Handling

Tool akan mengeluarkan error jika:

- File tidak ditemukan.
- File bukan PDF atau rusak.
- Halaman di luar rentang.
- Output path tidak valid.

## Unit Test

```powershell
pytest -q
```

## Contoh Output

Jika input `laporan.pdf` dan halaman `3`, maka output:

`laporan_halaman_3.pdf`

Jika input `laporan.pdf` dan range `1-3,8`, maka output:

`laporan_halaman_1-3.pdf` dan `laporan_halaman_8.pdf`
