import type { PanelAction, PanelDirection } from '../panel-tree/panel-tree';

// How each client lets the visitor work its Panels: the actions it offers as buttons, the keys the
// product binds to them (as the key caps show them), and how a key press maps to an action. The
// keys are the product's defaults: the terminal interface's after its prefix, Ctrl+b
// (apps/orchbee/crates/workbench-client/src/client_view/bindings.rs), and the desktop
// application's shortcuts.

export type PanelPlayClient = 'terminal' | 'desktop';

/** What a key press asks for. */
export type PanelCommand =
	| { readonly kind: 'act'; readonly act: PanelAction }
	| { readonly kind: 'move'; readonly where: PanelDirection }
	| { readonly kind: 'resize'; readonly where: PanelDirection }
	| { readonly kind: 'prefix' }
	| { readonly kind: 'leave' };

interface PanelPlayKeys {
	/** The actions offered as buttons, in order. */
	readonly actions: readonly PanelAction[];
	/** The key caps of each action that has a key, written as KeyCaps reads them. */
	readonly keys: Readonly<Partial<Record<PanelAction, string>>>;
	/** The key caps of moving a divider. */
	readonly resize: string;
	/** Whether the keys follow a prefix that the window acknowledges. */
	readonly prefix: boolean;
}

export const PANEL_PLAY_KEYS: Readonly<Record<PanelPlayClient, PanelPlayKeys>> = {
	terminal: {
		actions: ['right', 'down', 'left', 'up', 'zoom', 'close'],
		keys: { right: '|', down: '-', left: '\\', up: '_', zoom: 'z', close: 'Shift+X' },
		resize: 'Alt+← ↓ ↑ →',
		prefix: true,
	},
	desktop: {
		actions: ['right', 'down', 'zoom', 'equal', 'close'],
		keys: {
			right: 'Ctrl+Shift+D',
			down: 'Ctrl+Shift+E',
			zoom: 'Ctrl+Shift+Enter',
			equal: 'Ctrl+Alt+=',
		},
		resize: 'Alt+Shift+← ↓ ↑ →',
		prefix: false,
	},
};

const ARROWS: Readonly<Record<string, PanelDirection>> = {
	ArrowLeft: 'left',
	ArrowRight: 'right',
	ArrowUp: 'up',
	ArrowDown: 'down',
};
const VI_KEYS: Readonly<Record<string, PanelDirection>> = {
	h: 'left',
	j: 'down',
	k: 'up',
	l: 'right',
};
const TERMINAL_SPLITS: Readonly<Record<string, PanelDirection>> = {
	'|': 'right',
	'-': 'down',
	'\\': 'left',
	_: 'up',
};

type KeyPress = Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>;

// The window stands for the moment after the prefix: a key works with or without Ctrl+b first,
// and Ctrl+b itself only acknowledges the prefix.
function terminalCommand(event: KeyPress): PanelCommand | undefined {
	const { key } = event;
	const arrow = ARROWS[key];
	if (event.ctrlKey && key.toLowerCase() === 'b') return { kind: 'prefix' };
	if (event.ctrlKey || event.metaKey) return undefined;
	if (event.altKey && arrow !== undefined) return { kind: 'resize', where: arrow };
	if (arrow !== undefined) return { kind: 'move', where: arrow };
	const split = TERMINAL_SPLITS[key];
	if (split !== undefined) return { kind: 'act', act: split };
	if (key === 'z') return { kind: 'act', act: 'zoom' };
	if (key === 'X') return { kind: 'act', act: 'close' };
	const vi = VI_KEYS[key];
	return vi === undefined ? undefined : { kind: 'move', where: vi };
}

function desktopCommand(event: KeyPress): PanelCommand | undefined {
	const { key } = event;
	const arrow = ARROWS[key];
	if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 'd')
		return { kind: 'act', act: 'right' };
	if (event.ctrlKey && event.shiftKey && key.toLowerCase() === 'e')
		return { kind: 'act', act: 'down' };
	if (event.ctrlKey && event.shiftKey && key === 'Enter') return { kind: 'act', act: 'zoom' };
	if (event.ctrlKey && event.altKey && event.code === 'Equal') return { kind: 'act', act: 'equal' };
	if (event.ctrlKey && event.altKey && arrow !== undefined) return { kind: 'move', where: arrow };
	if (event.altKey && event.shiftKey && arrow !== undefined)
		return { kind: 'resize', where: arrow };
	return undefined;
}

/** What a key press in the window asks for, or undefined when the window leaves it alone. */
export function panelCommand(client: PanelPlayClient, event: KeyPress): PanelCommand | undefined {
	if (event.key === 'Escape') return { kind: 'leave' };
	return client === 'terminal' ? terminalCommand(event) : desktopCommand(event);
}
