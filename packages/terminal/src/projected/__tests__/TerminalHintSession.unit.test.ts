import { describe, expect, it, vi } from 'vitest';

import type { TerminalHint } from '../../terminal-hints';
import { terminalHintLabels } from '../../terminal-hints';
import { TerminalHintSession } from '../TerminalHintSession';

describe('TerminalHintSession', () => {
	it('filters equal-width labels and returns the selected typed target without leaking keys', async () => {
		const harness = createHarness();
		const caller = new AbortController();
		const session = new TerminalHintSession({
			canvas: harness.canvas,
			externalSignal: caller.signal,
			loadingText: 'Finding terminal hints…',
			root: harness.root,
		});
		const overlay = harness.onlyOverlay();
		expect(overlay.dataset['state']).toBe('loading');
		expect(overlay.children[0]?.textContent).toBe('Finding terminal hints…');
		const hints = Array.from({ length: 37 }, (_value, index) => terminalHint(index));
		const labels = terminalHintLabels(hints.length);

		expect(
			session.present({
				cellHeight: 18,
				cellWidth: 9,
				fontFamily: 'monospace',
				fontSize: 14,
				hints,
				labels,
				visibleStartRow: 5n,
			}),
		).toBe(true);
		expect(overlay.dataset['state']).toBe('active');
		expect(overlay.dataset['hintCount']).toBe('37');
		expect(overlay.children).toHaveLength(37);

		const firstPrefix = keyEvent('0');
		expect(session.handleKeyDown(firstPrefix.event)).toBe(true);
		expect(firstPrefix.preventDefault).toHaveBeenCalledOnce();
		expect(firstPrefix.stopPropagation).toHaveBeenCalledOnce();
		expect(overlay.dataset['prefix']).toBe('0');
		expect(overlay.children.at(-1)?.hidden).toBe(true);
		expect(overlay.children[10]?.textContent).toBe('a');

		const backspace = keyEvent('Backspace');
		session.handleKeyDown(backspace.event);
		expect(overlay.dataset['prefix']).toBe('');
		expect(overlay.children.at(-1)?.hidden).toBe(false);
		expect(overlay.children[10]?.textContent).toBe('0a');

		session.handleKeyDown(keyEvent('0').event);
		const selection = keyEvent('a');
		expect(session.handleKeyDown(selection.event)).toBe(true);
		await expect(session.completion).resolves.toEqual({ hint: hints[10], kind: 'selected' });
		expect(selection.preventDefault).toHaveBeenCalledOnce();
		expect(harness.children).toEqual([]);
		expect(session.active).toBe(false);
	});

	it('owns Escape while loading and cancels without an effect', async () => {
		const harness = createHarness();
		const session = new TerminalHintSession({
			canvas: harness.canvas,
			externalSignal: new AbortController().signal,
			root: harness.root,
		});
		const escape = keyEvent('Escape');

		expect(session.handleKeyDown(escape.event)).toBe(true);
		await expect(session.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(escape.preventDefault).toHaveBeenCalledOnce();
		expect(escape.stopPropagation).toHaveBeenCalledOnce();
		expect(harness.children).toEqual([]);
	});

	it('releases composition and external cancellation while removing the overlay', async () => {
		const compositionHarness = createHarness();
		const compositionSession = new TerminalHintSession({
			canvas: compositionHarness.canvas,
			externalSignal: new AbortController().signal,
			root: compositionHarness.root,
		});
		const composition = keyEvent('Process', { isComposing: true, keyCode: 229 });

		expect(compositionSession.handleKeyDown(composition.event)).toBe(false);
		await expect(compositionSession.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(composition.preventDefault).not.toHaveBeenCalled();
		expect(composition.stopPropagation).not.toHaveBeenCalled();
		expect(compositionHarness.children).toEqual([]);

		const caller = new AbortController();
		const externalHarness = createHarness();
		const externalSession = new TerminalHintSession({
			canvas: externalHarness.canvas,
			externalSignal: caller.signal,
			root: externalHarness.root,
		});
		caller.abort('replacement');

		await expect(externalSession.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(externalHarness.children).toEqual([]);
	});
});

interface Harness {
	readonly canvas: HTMLCanvasElement;
	readonly children: readonly FakeElement[];
	readonly root: HTMLElement;
	onlyOverlay(): FakeElement;
}

class FakeElement {
	readonly bounds: DOMRect;
	readonly children: FakeElement[] = [];
	readonly dataset = {} as DOMStringMap;
	readonly ownerDocument: Document;
	readonly style = {} as CSSStyleDeclaration;
	hidden = false;
	parent: FakeElement | undefined;
	textContent: string | null = null;

	constructor(ownerDocument: Document, bounds: DOMRect) {
		this.bounds = bounds;
		this.ownerDocument = ownerDocument;
	}

	appendChild<NodeType extends Node>(child: NodeType): NodeType {
		const element = child as unknown as FakeElement;
		element.parent = this;
		this.children.push(element);
		return child;
	}

	getBoundingClientRect(): DOMRect {
		return this.bounds;
	}

	querySelectorAll<ElementType extends Element = Element>(
		selector: string,
	): NodeListOf<ElementType> {
		const matches =
			selector === '[data-projected-terminal-hint="true"]'
				? this.children.filter((element) => element.dataset['projectedTerminalHint'] === 'true')
				: [];
		return matches as unknown as NodeListOf<ElementType>;
	}

	remove(): void {
		if (this.parent === undefined) return;
		const index = this.parent.children.indexOf(this);
		if (index >= 0) this.parent.children.splice(index, 1);
		this.parent = undefined;
	}

	replaceChildren(...nodes: Node[]): void {
		for (const child of this.children) child.parent = undefined;
		this.children.length = 0;
		for (const node of nodes) this.appendChild(node);
	}

	setAttribute(): void {}
}

function createHarness(): Harness {
	const rootBounds = rectangle(10, 20, 720, 360);
	const canvasBounds = rectangle(19, 38, 540, 180);
	let document: Document;
	document = {
		createElement: () => new FakeElement(document, rectangle(0, 0, 0, 0)),
	} as unknown as Document;
	const root = new FakeElement(document, rootBounds);
	const canvas = new FakeElement(document, canvasBounds);
	return {
		canvas: canvas as unknown as HTMLCanvasElement,
		children: root.children,
		onlyOverlay: () => {
			const overlay = root.children[0];
			if (overlay === undefined) throw new Error('Terminal hint overlay was not mounted.');
			return overlay;
		},
		root: root as unknown as HTMLElement,
	};
}

function terminalHint(index: number): TerminalHint {
	const text = index.toString(36).padStart(2, '0');
	return {
		kind: 'hash',
		range: { end: { x: index, y: 5 }, start: { x: index, y: 5 } },
		target: { kind: 'unknown', value: text },
		text,
	};
}

function keyEvent(
	key: string,
	overrides: { readonly isComposing?: boolean; readonly keyCode?: number } = {},
): {
	readonly event: KeyboardEvent;
	readonly preventDefault: ReturnType<typeof vi.fn>;
	readonly stopPropagation: ReturnType<typeof vi.fn>;
} {
	const preventDefault = vi.fn<() => void>();
	const stopPropagation = vi.fn<() => void>();
	return {
		event: {
			altKey: false,
			ctrlKey: false,
			isComposing: overrides.isComposing ?? false,
			key,
			keyCode: overrides.keyCode ?? 0,
			metaKey: false,
			preventDefault,
			repeat: false,
			stopPropagation,
		} as unknown as KeyboardEvent,
		preventDefault,
		stopPropagation,
	};
}

function rectangle(left: number, top: number, width: number, height: number): DOMRect {
	return {
		bottom: top + height,
		height,
		left,
		right: left + width,
		top,
		width,
		x: left,
		y: top,
		toJSON: () => ({}),
	};
}
