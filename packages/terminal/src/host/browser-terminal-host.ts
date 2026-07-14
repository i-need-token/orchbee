import type {
  ClipboardPayload,
  OpenTargetRequest,
  PasteConfirmationRequest,
  TerminalClipboardHost,
  TerminalHost,
} from "./terminal-host.js";

function getBrowserClipboard(): Clipboard | undefined {
  return globalThis.navigator?.clipboard;
}

async function writeBrowserClipboard(payload: ClipboardPayload): Promise<void> {
  const clipboard = getBrowserClipboard();
  if (!clipboard) throw new Error("The browser clipboard capability is unavailable");

  const ClipboardItemConstructor = globalThis.ClipboardItem;
  if (payload.html && ClipboardItemConstructor && clipboard.write) {
    const item = new ClipboardItemConstructor({
      "text/plain": new Blob([payload.text], { type: "text/plain" }),
      "text/html": new Blob([payload.html], { type: "text/html" }),
    });
    await clipboard.write([item]);
    return;
  }

  if (!clipboard.writeText) throw new Error("The browser text clipboard is unavailable");
  await clipboard.writeText(payload.text);
}

function createBrowserClipboardHost(): TerminalClipboardHost {
  return {
    async read() {
      const clipboard = getBrowserClipboard();
      if (!clipboard?.readText) throw new Error("The browser text clipboard is unavailable");
      const text = await clipboard.readText();
      return { text };
    },
    async write(request) {
      await writeBrowserClipboard(request.payload);
    },
  };
}

function getOpenTargetValue(request: OpenTargetRequest): string {
  switch (request.target.kind) {
    case "url":
      return request.target.url;
    case "unknown":
      return request.target.value;
    case "file":
      throw new Error("The browser host cannot open local file targets");
  }
}

function confirmBrowserPaste(request: PasteConfirmationRequest): Promise<boolean> {
  const confirm = globalThis.window?.confirm;
  if (!confirm) return Promise.resolve(false);
  const lineCount = request.text.split("\n").length;
  return Promise.resolve(confirm(`Paste ${lineCount} lines into the terminal?`));
}

async function openBrowserTarget(request: OpenTargetRequest): Promise<void> {
  const open = globalThis.window?.open;
  if (!open) throw new Error("The browser window capability is unavailable");
  const opened = open(getOpenTargetValue(request), "_blank", "noopener,noreferrer");
  if (opened === null) throw new Error("The browser blocked the terminal target");
}

export function createBrowserTerminalHost(): TerminalHost {
  const clipboard = getBrowserClipboard() ? createBrowserClipboardHost() : undefined;
  return {
    ...(clipboard ? { clipboard } : {}),
    capabilities: {
      clipboardRead: clipboard !== undefined,
      clipboardWrite: clipboard !== undefined,
      contextMenu: false,
      notifications: false,
      openTarget: globalThis.window?.open !== undefined,
      pasteConfirmation: globalThis.window?.confirm !== undefined,
      pointer: false,
      fileDrop: false,
    },
    confirmPaste: confirmBrowserPaste,
    openTarget: openBrowserTarget,
  };
}
