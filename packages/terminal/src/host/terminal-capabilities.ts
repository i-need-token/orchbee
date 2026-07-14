import type { TerminalHost, TerminalHostCapabilities } from "./terminal-host";

/** Resolve optional declarations against the methods actually supplied by a host. */
export function resolveHostCapabilities(host: TerminalHost): TerminalHostCapabilities {
  const declared = host.capabilities;
  return {
    clipboardRead: declared?.clipboardRead ?? host.clipboard !== undefined,
    clipboardWrite: declared?.clipboardWrite ?? host.clipboard !== undefined,
    contextMenu: declared?.contextMenu ?? host.showContextMenu !== undefined,
    notifications: declared?.notifications ?? host.notify !== undefined,
    openTarget: declared?.openTarget ?? host.openTarget !== undefined,
    pasteConfirmation: declared?.pasteConfirmation ?? host.confirmPaste !== undefined,
    pointer: declared?.pointer ?? host.setPointer !== undefined,
    fileDrop: declared?.fileDrop ?? host.resolveDrop !== undefined,
  };
}
