import type { ActionSignature } from '@orchbee/actions';

export const TERMINAL_ACTION_IDS = {
	clear: 'terminal.clear',
	copySelection: 'terminal.selection.copy',
	findNext: 'terminal.search.next',
	findOpen: 'terminal.search.open',
	findPrevious: 'terminal.search.previous',
	openLink: 'terminal.link.open',
	paste: 'terminal.clipboard.paste',
	reset: 'terminal.reset',
	scrollToBottom: 'terminal.history.scroll-bottom',
	selectAll: 'terminal.selection.select-all',
} as const;

export type WorkbenchTerminalActionId =
	(typeof TERMINAL_ACTION_IDS)[keyof typeof TERMINAL_ACTION_IDS];

/** Revision-pinned link facts captured when a terminal context menu opens. */
export interface WorkbenchTerminalLinkActionInput {
	readonly context?:
		| {
				readonly column: number;
				readonly generation: string;
				readonly linkEpoch: string;
				readonly revision: string;
				readonly row: string;
		  }
		| undefined;
	readonly target:
		| {
				readonly column?: number | undefined;
				readonly kind: 'file';
				readonly line?: number | undefined;
				readonly path: string;
		  }
		| { readonly kind: 'unknown'; readonly value: string }
		| { readonly kind: 'url'; readonly url: string };
}

/** Minimal terminal intent catalog owned by the containing Workbench. */
export interface WorkbenchTerminalActionCatalog {
	readonly 'terminal.clear': ActionSignature<void>;
	readonly 'terminal.clipboard.paste': ActionSignature<void>;
	readonly 'terminal.history.scroll-bottom': ActionSignature<void>;
	readonly 'terminal.link.open': ActionSignature<WorkbenchTerminalLinkActionInput>;
	readonly 'terminal.reset': ActionSignature<void>;
	readonly 'terminal.search.next': ActionSignature<void>;
	readonly 'terminal.search.open': ActionSignature<void>;
	readonly 'terminal.search.previous': ActionSignature<void>;
	readonly 'terminal.selection.copy': ActionSignature<void>;
	readonly 'terminal.selection.select-all': ActionSignature<void>;
}
