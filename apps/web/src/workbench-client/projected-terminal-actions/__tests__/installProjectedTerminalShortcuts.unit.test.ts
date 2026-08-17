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

	it('keeps unshifted Standard Ctrl+C, Ctrl+A, and Ctrl+F available to the PTY', () => {
		const harness = shortcutHarness('standard');
		for (const code of ['KeyC', 'KeyA', 'KeyF']) {
			harness.keyDown({ ...baseEvent, code, ctrlKey: true }, harness.terminalNode);
		}
		expect(harness.invoke).not.toHaveBeenCalled();

		for (const code of ['KeyC', 'KeyA']) {
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
});

interface ShortcutHarness {
	readonly findNode: TestNode;
	readonly findScope: ActionScope;
	readonly invoke: ReturnType<
		typeof vi.fn<(actionId: string, input: undefined, scope: ActionScope) => boolean>
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
	const invoke = vi.fn<(actionId: string, input: undefined, scope: ActionScope) => boolean>(
		() => available,
	);
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
