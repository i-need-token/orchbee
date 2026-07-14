import type { ContextMenuItem } from "../context-menu/context-menu-registry";

export type ClipboardTarget = "standard" | "selection";

export type ClipboardReadSource = "paste" | "middle-click" | "osc52" | "api";

export type ClipboardWriteSource = "copy" | "copy-on-select" | "osc52" | "api";

export interface ClipboardPayload {
  readonly text: string;
  readonly html?: string;
  readonly vt?: string;
}

export interface ClipboardReadRequest {
  readonly target: ClipboardTarget;
  readonly source: ClipboardReadSource;
}

export interface ClipboardWriteRequest {
  readonly target: ClipboardTarget;
  readonly source: ClipboardWriteSource;
  readonly payload: ClipboardPayload;
}

export interface TerminalClipboardHost {
  read(request: ClipboardReadRequest): Promise<ClipboardPayload | null>;
  write(request: ClipboardWriteRequest): Promise<void>;
}

export type TerminalOpenTarget =
  | { readonly kind: "url"; readonly url: string }
  | {
      readonly kind: "file";
      readonly path: string;
      readonly line?: number;
      readonly column?: number;
    }
  | { readonly kind: "unknown"; readonly value: string };

export interface OpenTargetRequest {
  readonly target: TerminalOpenTarget;
  readonly source: "link" | "context-menu" | "api";
}

export interface PasteConfirmationRequest {
  readonly text: string;
  readonly sanitized: string;
  readonly bracketed: boolean;
  readonly multiline: boolean;
  readonly embeddedBracketTerminator: boolean;
  readonly replacedControlBytes: number;
}

export interface TerminalDropRequest {
  readonly text: string;
  readonly files: readonly File[];
  readonly cwd: string | null;
}

export interface TerminalDropResolution {
  /** Shell-ready text after the host applies file-path and quoting policy. */
  readonly text: string;
}

export interface TerminalModifierState {
  readonly alt: boolean;
  readonly control: boolean;
  readonly meta: boolean;
  readonly shift: boolean;
}

export interface TerminalSelectionSnapshot {
  readonly shape: "linear" | "rectangle";
  readonly start: { readonly column: number; readonly row: number };
  readonly end: { readonly column: number; readonly row: number };
  readonly text: string;
}

export interface TerminalInteractionContext {
  readonly cell: {
    readonly column: number;
    readonly row: number;
    readonly bufferRow: number;
  } | null;
  readonly selection: TerminalSelectionSnapshot | null;
  readonly link: {
    readonly text: string;
    readonly target: TerminalOpenTarget | null;
  } | null;
  readonly semanticZone: "prompt" | "input" | "output" | null;
  readonly cwd: string | null;
  readonly modifiers: TerminalModifierState;
  readonly mouseReporting: boolean;
}

export interface ContextMenuActionState {
  readonly id: string;
  readonly title: string;
  readonly enabled: boolean;
  readonly checked: boolean;
}

export interface ContextMenuRequest {
  readonly x: number;
  readonly y: number;
  readonly actionIds: readonly string[];
  readonly items: readonly ContextMenuItem[];
  readonly actions: readonly ContextMenuActionState[];
  readonly context: TerminalInteractionContext;
}

export interface TerminalNotificationRequest {
  readonly title: string;
  readonly body: string;
}

export interface PointerRequest {
  readonly shape: "default" | "text" | "pointer" | "crosshair" | "not-allowed";
  readonly visible: boolean;
}

export interface TerminalHostCapabilities {
  readonly clipboardRead: boolean;
  readonly clipboardWrite: boolean;
  readonly contextMenu: boolean;
  readonly notifications: boolean;
  readonly openTarget: boolean;
  readonly pasteConfirmation: boolean;
  readonly pointer: boolean;
  readonly fileDrop: boolean;
}

export interface TerminalHost {
  readonly capabilities?: Partial<TerminalHostCapabilities>;
  readonly clipboard?: TerminalClipboardHost;
  confirmPaste?(request: PasteConfirmationRequest): Promise<boolean>;
  resolveDrop?(request: TerminalDropRequest): Promise<TerminalDropResolution | null>;
  openTarget?(request: OpenTargetRequest): Promise<void>;
  showContextMenu?(request: ContextMenuRequest): Promise<string | null>;
  notify?(request: TerminalNotificationRequest): Promise<void>;
  setPointer?(request: PointerRequest): void;
}

export type TerminalHostOperation =
  | "clipboard-read"
  | "clipboard-write"
  | "confirm-paste"
  | "resolve-drop"
  | "open-target"
  | "context-menu"
  | "notification"
  | "font-load"
  | "input-encode"
  | "input-dispatch";

export interface TerminalHostErrorEvent {
  readonly operation: TerminalHostOperation;
  readonly error: unknown;
}
