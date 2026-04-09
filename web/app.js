const dropZone = document.getElementById("dropZone");
const fileInput = document.getElementById("fileInput");
const ocrForm = document.getElementById("ocrForm");
const langInput = document.getElementById("langInput");
const runBtn = document.getElementById("runBtn");
const fileMeta = document.getElementById("fileMeta");
const statusEl = document.getElementById("status");
const resultPanel = document.getElementById("resultPanel");
const statsEl = document.getElementById("stats");
const resultList = document.getElementById("resultList");
const copyAllBtn = document.getElementById("copyAllBtn");

const compressBtn = document.getElementById("compressBtn");

const state = {
  file: null,
  ocrResult: null,
  loadingTimer: null,
  isRunning: false,
};

function formatBytes(bytes) {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let idx = 0;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx += 1;
  }
  return `${value.toFixed(idx === 0 ? 0 : 2)} ${units[idx]}`;
}

function setStatus(message, type = "") {
  statusEl.className = `status ${type}`.trim();
  statusEl.textContent = message;
}

function startLoadingStatus(message = "Memproses OCR") {
  const frames = [`${message}.`, `${message}..`, `${message}...`];
  let i = 0;
  setStatus(frames[0]);
  state.loadingTimer = window.setInterval(() => {
    i = (i + 1) % frames.length;
    setStatus(frames[i]);
  }, 280);
}

function stopLoadingStatus() {
  if (state.loadingTimer) {
    window.clearInterval(state.loadingTimer);
    state.loadingTimer = null;
  }
}

function setFile(file) {
  state.file = file;
  state.ocrResult = null;
  resultPanel.classList.add("hidden");
  fileMeta.classList.remove("empty");
  fileMeta.textContent = `File: ${file.name} | Size: ${formatBytes(file.size)} | Type: ${file.type || "unknown"}`;
  setStatus("File siap diproses.", "ok");
}

function safeErrorMessage(error) {
  if (!error) {
    return "Terjadi kesalahan saat OCR.";
  }
  if (typeof error === "string") {
    return error;
  }
  if (error.message) {
    return error.message;
  }
  return "Terjadi kesalahan saat OCR.";
}

function extractBackendError(rawText) {
  if (!rawText) {
    return "";
  }
  try {
    const payload = JSON.parse(rawText);
    if (payload && payload.detail) {
      return String(payload.detail);
    }
  } catch (parseError) {
    return rawText.slice(0, 300);
  }
  return "";
}

function handleFiles(fileList) {
  const file = fileList && fileList[0];
  if (!file) {
    return;
  }
  setFile(file);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    setStatus("Teks berhasil disalin.", "ok");
  } catch (error) {
    setStatus("Gagal menyalin teks.", "error");
  }
}

function renderStats(result) {
  statsEl.innerHTML = "";
  const chips = [
    `File: ${result.filename}`,
    `Jenis: ${result.kind.toUpperCase()}`,
    `Bahasa: ${result.lang}`,
    `Halaman: ${result.page_count}`,
    `Karakter: ${result.char_count}`,
  ];
  if (Array.isArray(result.missing_langs) && result.missing_langs.length > 0) {
    chips.push(`Language tidak tersedia: ${result.missing_langs.join(", ")}`);
  }
  for (const chipText of chips) {
    const chip = document.createElement("div");
    chip.className = "stat-chip";
    chip.textContent = chipText;
    statsEl.appendChild(chip);
  }
}

function renderResultList(pages) {
  resultList.innerHTML = "";
  for (const pageData of pages) {
    const card = document.createElement("article");
    card.className = "result-card";

    const head = document.createElement("div");
    head.className = "result-card-head";

    const title = document.createElement("h3");
    title.textContent = `Halaman ${pageData.page} (${pageData.char_count} chars)`;
    head.appendChild(title);

    const copyBtn = document.createElement("button");
    copyBtn.type = "button";
    copyBtn.textContent = "Copy";
    copyBtn.addEventListener("click", () => copyText(pageData.text || ""));
    head.appendChild(copyBtn);

    const pre = document.createElement("pre");
    pre.textContent = pageData.text || "(tidak ada teks terdeteksi)";

    card.appendChild(head);
    card.appendChild(pre);
    resultList.appendChild(card);
  }
}

function renderResult(result) {
  renderStats(result);
  renderResultList(result.pages || []);
  resultPanel.classList.remove("hidden");
}

dropZone.addEventListener("click", () => fileInput.click());
dropZone.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    fileInput.click();
  }
});

dropZone.addEventListener("dragover", (event) => {
  event.preventDefault();
  dropZone.classList.add("active");
});

dropZone.addEventListener("dragleave", () => {
  dropZone.classList.remove("active");
});

dropZone.addEventListener("drop", (event) => {
  event.preventDefault();
  dropZone.classList.remove("active");
  handleFiles(event.dataTransfer.files);
});

fileInput.addEventListener("change", (event) => {
  handleFiles(event.target.files);
});

copyAllBtn.addEventListener("click", () => {
  if (!state.ocrResult) {
    setStatus("Belum ada hasil untuk disalin.", "error");
    return;
  }
  const merged = (state.ocrResult.pages || [])
    .map((p) => `--- Halaman ${p.page} ---\n${p.text || ""}`)
    .join("\n\n");
  copyText(merged);
});

async function runOCR() {
  if (state.isRunning) {
    return;
  }

  if (!state.file) {
    setStatus("Pilih file dulu sebelum OCR.", "error");
    return;
  }

  const formData = new FormData();
  formData.append("file", state.file);
  formData.append("lang", langInput.value || "eng");

  stopLoadingStatus();
  state.isRunning = true;
  runBtn.disabled = true;
  startLoadingStatus();
  try {
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 30 * 60 * 1000);
    const response = await fetch("/api/ocr", {
      method: "POST",
      body: formData,
      signal: controller.signal,
    });
    window.clearTimeout(timeoutId);
    const rawText = await response.text();
    let payload = {};
    try {
      payload = rawText ? JSON.parse(rawText) : {};
    } catch (parseError) {
      payload = {};
    }
    if (!response.ok) {
      const message =
        (payload && payload.detail) ||
        extractBackendError(rawText) ||
        "OCR gagal diproses.";
      throw new Error(message);
    }
    state.ocrResult = payload;
    renderResult(payload);
    setStatus("OCR selesai.", "ok");
  } catch (error) {
    console.error("OCR request error:", error);
    setStatus(safeErrorMessage(error), "error");
  } finally {
    stopLoadingStatus();
    state.isRunning = false;
    runBtn.disabled = false;
  }
}

async function runCompress() {
  if (state.isRunning) {
    return;
  }

  if (!state.file) {
    setStatus("Pilih file dulu sebelum kompresi.", "error");
    return;
  }

  if (state.file.type !== "application/pdf" && !state.file.name.toLowerCase().endsWith(".pdf")) {
    setStatus("Hanya file PDF yang bisa dikompresi.", "error");
    return;
  }

  const formData = new FormData();
  formData.append("file", state.file);

  stopLoadingStatus();
  state.isRunning = true;
  runBtn.disabled = true;
  compressBtn.disabled = true;
  startLoadingStatus("Sedang mengompresi PDF");
  try {
    const controller = new AbortController();
    const timeoutId = window.setTimeout(() => controller.abort(), 10 * 60 * 1000);
    const response = await fetch("/api/compress", {
      method: "POST",
      body: formData,
      signal: controller.signal,
    });
    window.clearTimeout(timeoutId);

    if (!response.ok) {
      const rawText = await response.text();
      const message = extractBackendError(rawText) || "Kompresi gagal.";
      throw new Error(message);
    }

    const blob = await response.blob();
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    
    // Get filename from header or fallback
    let filename = state.file.name.replace(".pdf", "_compressed.pdf");
    const disposition = response.headers.get("Content-Disposition");
    if (disposition && (disposition.indexOf("filename=") !== -1)) {
        const parts = disposition.split(";");
        for (let part of parts) {
            if (part.trim().startsWith("filename=")) {
                filename = part.split("=")[1].trim().replace(/"/g, "");
            }
        }
    }
    
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    window.URL.revokeObjectURL(url);
    a.remove();

    setStatus(`Berhasil! PDF dikompresi menjadi ${formatBytes(blob.size)}.`, "ok");
  } catch (error) {
    console.error("Compression request error:", error);
    setStatus(safeErrorMessage(error), "error");
  } finally {
    stopLoadingStatus();
    state.isRunning = false;
    runBtn.disabled = false;
    compressBtn.disabled = false;
  }
}

ocrForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await runOCR();
});

runBtn.addEventListener("click", async (event) => {
  event.preventDefault();
  await runOCR();
});

compressBtn.addEventListener("click", async (event) => {
  event.preventDefault();
  await runCompress();
});

window.addEventListener("error", (event) => {
  console.error("Frontend error:", event.error || event.message);
  setStatus(
    `Frontend error: ${event.message || "Unknown error. Cek DevTools Console."}`,
    "error"
  );
});

window.addEventListener("unhandledrejection", (event) => {
  console.error("Unhandled promise rejection:", event.reason);
  setStatus("Frontend promise error. Cek DevTools Console.", "error");
});

setStatus("UI siap. Pilih file lalu klik Run OCR atau Run Compress.", "ok");
