import { ActionRuntime, type ActionScope } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopTerminalShortcuts,
	type DesktopShortcutPlatform,
} from '../installDesktopTerminalShortcuts';

const baseEvent = {
	altKey: false,
	code: 'KeyF',
	ctrlKey: false,
	isComposing: false,
	metaKey: false,
	repeat: false,
	shiftKey: false,
};

describe('desktop terminal shortcuts', () => {
	it('opens find with the platform primary modifier in terminal or find focus', () => {
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
		standard.keyDown({ ...baseEvent, ctrlKey: true }, standard.terminalNode);
		expect(standard.invoke).toHaveBeenCalledWith(
			'terminal.search.open',
			undefined,
			standard.terminalScope,
		);
	});

	it('keeps non-Apple Ctrl+C and Ctrl+A available to the PTY', () => {
		const harness = shortcutHarness('standard');
		harness.keyDown({ ...baseEvent, code: 'KeyC', ctrlKey: true }, harness.terminalNode);
		harness.keyDown({ ...baseEvent, code: 'KeyA', ctrlKey: true }, harness.terminalNode);
		expect(harness.invoke).not.toHaveBeenCalled();

		harness.keyDown(
			{ ...baseEvent, code: 'KeyC', ctrlKey: true, shiftKey: true },
			harness.terminalNode,
		);
		expect(harness.invoke).toHaveBeenCalledWith(
			'terminal.selection.copy',
			undefined,
			harness.terminalScope,
		);
	});
});

interface ShortcutHarness {
	readonly findNode: TestNode;
	readonly findScope: ActionScope;
	readonly invoke: ReturnType<
		typeof vi.fn<(actionId: string, input: undefined, scope: ActionScope) => void>
	>;
	readonly terminalNode: TestNode;
	readonly terminalScope: ActionScope;
	keyDown(event: typeof baseEvent, target: TestNode): void;
}

class TestNode {}

function shortcutHarness(platform: DesktopShortcutPlatform): ShortcutHarness {
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
	const invoke = vi.fn<(actionId: string, input: undefined, scope: ActionScope) => void>();
	installDesktopTerminalShortcuts({
		document,
		findRoot,
		findScope,
		invoke,
		platform,
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
			listener({
				...event,
				preventDefault: vi.fn<() => void>(),
				stopImmediatePropagation: vi.fn<() => void>(),
				target,
			} as unknown as KeyboardEvent);
		},
		terminalNode,
		terminalScope,
	};
}
