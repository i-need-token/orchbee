import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import { installProjectedTerminalShortcuts } from '../installProjectedTerminalShortcuts';
import {
	defaultProjectedTerminalShortcutBindings,
	type ProjectedTerminalShortcutBindings,
	type ProjectedTerminalShortcutPlatform,
} from '../../../projected-terminal-keyboard-shortcuts';

const baseEvent = {
	altKey: false,
	code: 'KeyF',
	ctrlKey: false,
	isComposing: false,
	metaKey: false,
	repeat: false,
	shiftKey: false,
};

describe('projected terminal shortcuts', () => {
	it('opens find with the platform default in terminal or find focus', () => {
		const apple = shortcutHarness('apple');
		apple.keyDown({ ...baseEvent, metaKey: true }, apple.terminalNode);
		apple.keyDown({ ...baseEvent, metaKey: true }, apple.findNode);
		expect(apple.invoke).toHaveBeenNthCalledWith(
			1,
			'terminal.search.open',
			undefined,
			apple.terminalScope,
		);
		expect(apple.invoke).toHaveBeenNthCalledWith(
			2,
			'terminal.search.open',
			undefined,
			apple.findScope,
		);

		const standard = shortcutHarness('standard');
		standard.keyDown({ ...baseEvent, ctrlKey: true, shiftKey: true }, standard.terminalNode);
		expect(standard.invoke).toHaveBeenCalledWith(
			'terminal.search.open',
			undefined,
			standard.terminalScope,
		);
	});

	it('clears only the focused terminal with the platform default', () => {
		const apple = shortcutHarness('apple');
		const accepted = apple.keyDown(
			{ ...baseEvent, code: 'KeyK', metaKey: true },
			apple.terminalNode,
		);
		expect(apple.invoke).toHaveBeenCalledWith('terminal.clear', undefined, apple.terminalScope);
		expect(accepted.preventDefault).toHaveBeenCalledOnce();

		apple.keyDown({ ...baseEvent, code: 'KeyK', metaKey: true }, apple.findNode);
		expect(apple.invoke).toHaveBeenCalledOnce();

		const standard = shortcutHarness('standard');
		standard.keyDown(
			{ ...baseEvent, code: 'KeyL', ctrlKey: true, shiftKey: true },
			standard.terminalNode,
		);
		expect(standard.invoke).toHaveBeenCalledWith(
			'terminal.clear',
			undefined,
			standard.terminalScope,
		);
	});

	it('routes prompt navigation from terminal focus', () => {
		const harness = shortcutHarness('apple');
		const previous = harness.keyDown(
			{ ...baseEvent, code: 'KeyZ', ctrlKey: true, shiftKey: true },
			harness.terminalNode,
		);
		const next = harness.keyDown(
			{ ...baseEvent, code: 'KeyX', ctrlKey: true, shiftKey: true },
			harness.terminalNode,
		);

		expect(harness.invoke).toHaveBeenNthCalledWith(
			1,
			'terminal.history.prompt-previous',
			undefined,
			harness.terminalScope,
		);
		expect(harness.invoke).toHaveBeenNthCalledWith(
			2,
			'terminal.history.prompt-next',
			undefined,
			harness.terminalScope,
		);
		expect(previous.preventDefault).toHaveBeenCalledOnce();
		expect(next.preventDefault).toHaveBeenCalledOnce();

		harness.keyDown(
			{ ...baseEvent, code: 'KeyZ', ctrlKey: true, shiftKey: true },
			harness.findNode,
		);
		expect(harness.invoke).toHaveBeenCalledTimes(2);
	});

	it('routes history scrolling and leaves unavailable bindings unconsumed', () => {
		const harness = shortcutHarness('standard');
		const shortcuts = [
			['ArrowUp', 'terminal.history.scroll-line-up'],
			['ArrowDown', 'terminal.history.scroll-line-down'],
			['PageUp', 'terminal.history.scroll-page-up'],
			['PageDown', 'terminal.history.scroll-page-down'],
			['Home', 'terminal.history.scroll-top'],
			['End', 'terminal.history.scroll-bottom'],
		] as const;
		for (const [code] of shortcuts) {
			const event = harness.keyDown(
				{ ...baseEvent, code, ctrlKey: true, shiftKey: true },
				harness.terminalNode,
			);
			expect(event.preventDefault).toHaveBeenCalledOnce();
		}
		expect(harness.invoke.mock.calls.map(([actionId]) => actionId)).toEqual(
			shortcuts.map(([, actionId]) => actionId),
		);

		const unavailable = shortcutHarness('standard', false);
		const declined = unavailable.keyDown(
			{ ...baseEvent, code: 'PageUp', ctrlKey: true, shiftKey: true },
			unavailable.terminalNode,
		);
		expect(unavailable.invoke).toHaveBeenCalledWith(
			'terminal.history.scroll-page-up',
			undefined,
			unavailable.terminalScope,
		);
		expect(declined.preventDefault).not.toHaveBeenCalled();
		expect(declined.stopPropagation).not.toHaveBeenCalled();
	});

	it('keeps unshifted Standard Ctrl+C, Ctrl+A, Ctrl+F, Ctrl+L, and Ctrl+V available to the PTY', () => {
		const harness = shortcutHarness('standard');
		for (const code of ['KeyC', 'KeyA', 'KeyF', 'KeyL', 'KeyV']) {
			harness.keyDown({ ...baseEvent, code, ctrlKey: true }, harness.terminalNode);
		}
		expect(harness.invoke).not.toHaveBeenCalled();

		for (const code of ['KeyC', 'KeyA', 'KeyV']) {
			harness.keyDown({ ...baseEvent, code, ctrlKey: true, shiftKey: true }, harness.terminalNode);
		}
		expect(harness.invoke).toHaveBeenNthCalledWith(
			1,
			'terminal.selection.copy',
			undefined,
			harness.terminalScope,
		);
		expect(harness.invoke).toHaveBeenNthCalledWith(
			2,
			'terminal.selection.select-all',
			undefined,
			harness.terminalScope,
		);
		expect(harness.invoke).toHaveBeenNthCalledWith(
			3,
			'terminal.clipboard.paste',
			undefined,
			harness.terminalScope,
			{ nativePasteFallback: true },
		);
	});

	it('consumes only a non-repeated binding whose Action is currently enabled', () => {
		const unavailable = shortcutHarness('apple', false);
		const declined = unavailable.keyDown({ ...baseEvent, metaKey: true }, unavailable.terminalNode);
		expect(unavailable.invoke).toHaveBeenCalledOnce();
		expect(declined.preventDefault).not.toHaveBeenCalled();
		expect(declined.stopPropagation).not.toHaveBeenCalled();

		const available = shortcutHarness('apple');
		const repeated = available.keyDown(
			{ ...baseEvent, metaKey: true, repeat: true },
			available.terminalNode,
		);
		expect(available.invoke).not.toHaveBeenCalled();
		expect(repeated.preventDefault).not.toHaveBeenCalled();

		const accepted = available.keyDown({ ...baseEvent, metaKey: true }, available.terminalNode);
		expect(accepted.preventDefault).toHaveBeenCalledOnce();
		expect(accepted.stopPropagation).toHaveBeenCalledOnce();
	});

	it('reads the latest caller-owned binding table for every event', () => {
		let bindings: ProjectedTerminalShortcutBindings =
			defaultProjectedTerminalShortcutBindings('standard');
		const harness = shortcutHarness('standard', true, () => bindings);
		bindings = Object.freeze({
			...bindings,
			'terminal.search.open': Object.freeze({
				code: 'KeyG',
				modifiers: Object.freeze(['mod', 'shift'] as const),
			}),
		});

		harness.keyDown(
			{ ...baseEvent, code: 'KeyG', ctrlKey: true, shiftKey: true },
			harness.terminalNode,
		);
		expect(harness.invoke).toHaveBeenCalledWith(
			'terminal.search.open',
			undefined,
			harness.terminalScope,
		);
	});

	it('offers native fallback only for the platform-default Paste chord', () => {
		const apple = shortcutHarness('apple');
		const native = apple.keyDown({ ...baseEvent, code: 'KeyV', metaKey: true }, apple.terminalNode);
		expect(apple.invoke).toHaveBeenCalledWith(
			'terminal.clipboard.paste',
			undefined,
			apple.terminalScope,
			{ nativePasteFallback: true },
		);
		expect(native.preventDefault).toHaveBeenCalledOnce();

		const customBindings = {
			...defaultProjectedTerminalShortcutBindings('apple'),
			'terminal.clipboard.paste': {
				code: 'KeyP',
				modifiers: ['mod', 'shift'] as const,
			},
		};
		const custom = shortcutHarness('apple', true, () => customBindings);
		custom.keyDown(
			{ ...baseEvent, code: 'KeyP', metaKey: true, shiftKey: true },
			custom.terminalNode,
		);
		expect(custom.invoke).toHaveBeenCalledWith(
			'terminal.clipboard.paste',
			undefined,
			custom.terminalScope,
		);
	});
});

interface ShortcutHarness {
	readonly findNode: TestNode;
	readonly findScope: ActionScope;
	readonly invoke: ReturnType<
		typeof vi.fn<
			(
				actionId: string,
				input: undefined,
				scope: ActionScope,
				options?: { readonly nativePasteFallback: true },
			) => boolean
		>
	>;
	readonly terminalNode: TestNode;
	readonly terminalScope: ActionScope;
	keyDown(
		event: typeof baseEvent,
		target: TestNode,
	): {
		readonly preventDefault: ReturnType<typeof vi.fn<() => void>>;
		readonly stopPropagation: ReturnType<typeof vi.fn<() => void>>;
	};
}

class TestNode {}

function shortcutHarness(
	platform: ProjectedTerminalShortcutPlatform,
	available = true,
	currentBindings?: () => ProjectedTerminalShortcutBindings,
): ShortcutHarness {
	const listeners = new Map<string, EventListener>();
	const document = {
		addEventListener: (type: string, listener: EventListenerOrEventListenerObject | null): void => {
			if (typeof listener === 'function') listeners.set(type, listener);
		},
		removeEventListener: (type: string): void => {
			listeners.delete(type);
		},
	} as unknown as Document;
	const ownerDocument = { defaultView: { Node: TestNode } };
	const findNode = new TestNode();
	const terminalNode = new TestNode();
	const findRoot = {
		contains: (node: Node) => node === findNode,
		ownerDocument,
	} as unknown as HTMLElement;
	const terminalRoot = {
		contains: (node: Node) => node === terminalNode,
		ownerDocument,
	} as unknown as HTMLElement;
	const runtime = new ActionRuntime();
	const terminalScope = runtime.createScope('terminal');
	const findScope = runtime.createScope('find', terminalScope);
	const invoke = vi.fn<
		(
			actionId: string,
			input: undefined,
			scope: ActionScope,
			options?: { readonly nativePasteFallback: true },
		) => boolean
	>(() => available);
	installProjectedTerminalShortcuts({
		document,
		findRoot,
		findScope,
		invoke,
		platform,
		...(currentBindings === undefined
			? {}
			: { shortcutBindingSource: { current: currentBindings } }),
		terminalRoot,
		terminalScope,
	});

	return {
		findNode,
		findScope,
		invoke,
		keyDown: (event, target) => {
			const listener = listeners.get('keydown');
			if (listener === undefined) throw new Error('keydown listener is unavailable');
			const preventDefault = vi.fn<() => void>();
			const stopPropagation = vi.fn<() => void>();
			listener({
				...event,
				preventDefault,
				stopPropagation,
				target,
			} as unknown as KeyboardEvent);
			return { preventDefault, stopPropagation };
		},
		terminalNode,
		terminalScope,
	};
}
