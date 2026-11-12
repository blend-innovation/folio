import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type { Bootstrap, DocumentPayload } from "./types";

export const hasTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * In a plain browser (e2e, screenshots, design preview) the UI is driven by a
 * fixture generated from the real Rust renderer. The native app uses Tauri IPC.
 */
let fixture: Bootstrap | null = null;

async function getFixture(): Promise<Bootstrap> {
  if (fixture) return fixture;
  const response = await fetch("fixtures/bootstrap.json", { cache: "no-store" });
  if (!response.ok) {
    throw new Error("Preview fixture missing. Run `node scripts/fixtures.mjs`.");
  }
  fixture = (await response.json()) as Bootstrap;
  return fixture;
}

export async function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (hasTauri()) {
    return tauriInvoke<T>(command, args);
  }

  const data = await getFixture();
  switch (command) {
    case "bootstrap":
      return data as unknown as T;
    case "open_sample":
      return (data.sampleDocument ?? data.initialDocument) as unknown as T;
    case "open_file_dialog":
      return null as unknown as T;
    case "render_document": {
      const path = String(args?.path ?? "");
      if (path === ":sample" || path === data.initialDocument?.path) {
        return (data.sampleDocument ?? data.initialDocument) as unknown as T;
      }
      throw new Error("The web preview cannot read files. Open the native app.");
    }
    case "read_image":
      throw new Error("Local images are only available in the native app.");
    case "reveal_themes":
    case "open_external":
    case "open_path":
    case "show_window":
    case "set_title":
      return undefined as unknown as T;
    case "markdown_default_status":
      return { available: false, isDefault: false } as unknown as T;
    case "set_markdown_default":
      throw new Error("Setting the default app is only available in the native app.");
    default:
      throw new Error(`Unknown preview command: ${command}`);
  }
}

export async function listen<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (!hasTauri()) {
    return () => {};
  }
  return tauriListen<T>(event, (message) => handler(message.payload));
}

export async function openDocumentPath(
  path: string,
  anchor?: string,
): Promise<DocumentPayload | null> {
  const payload = await invoke<DocumentPayload | null>("render_document", {
    path,
    anchor: anchor ?? null,
  });
  return payload ?? null;
}
