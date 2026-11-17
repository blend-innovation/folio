import { invoke } from "@tauri-apps/api/core";
import { mountPreview } from "./preview-frame";

const frame = document.getElementById("preview-frame");
const status = document.getElementById("preview-status");

function showStatus(message: string): void {
  if (status) {
    status.textContent = message;
    status.hidden = false;
  }
}

async function boot(): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) {
    showStatus("Open this preview from Folio.");
    return;
  }
  if (!(frame instanceof HTMLIFrameElement)) {
    showStatus("The preview frame is unavailable.");
    return;
  }

  try {
    const source = await invoke<string | null>("html_preview_source");
    if (!source) {
      showStatus("No HTML preview is available.");
      return;
    }
    mountPreview(frame, source);
  } catch (error) {
    showStatus(
      error instanceof Error ? error.message : "The preview could not be loaded.",
    );
  }
}

void boot();
