export { createBrowserTerminalHost } from './browser-terminal-host';
export { resolveHostCapabilities } from './terminal-capabilities';
export type {
	ClipboardPayload,
	ClipboardReadRequest,
	ClipboardReadSource,
	ClipboardTarget,
	ClipboardWriteRequest,
	ClipboardWriteSource,
	ContextMenuActionState,
	ContextMenuRequest,
	OpenTargetRequest,
	PasteConfirmationRequest,
	PointerRequest,
	TerminalClipboardHost,
	TerminalDropRequest,
	TerminalDropResolution,
	TerminalHost,
	TerminalHostCapabilities,
	TerminalHostErrorEvent,
	TerminalHostOperation,
	TerminalInteractionContext,
	TerminalModifierState,
	TerminalNotificationRequest,
	TerminalSelectionSnapshot,
} from './terminal-host';
export type { TerminalOpenTarget } from '../terminal-contracts';
