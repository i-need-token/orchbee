import { afterEach, describe, expect, it, vi } from 'vitest';

import { activateWorkbenchContextMenuOutsideTarget } from '../activateWorkbenchContextMenuOutsideTarget';

class TestNode {
	readonly ownerDocument: {
		readonly elementsFromPoint: (clientX: number, clientY: number) => readonly unknown[];
	};

	constructor(ownerDocument: {
		readonly elementsFromPoint: (clientX: number, clientY: number) => readonly unknown[];
	}) {
		this.ownerDocument = ownerDocument;
	}
}

class TestMouseEvent {
	readonly button: number;
	readonly clientX: number;
	readonly clientY: number;
	readonly target: TestNode;

	constructor(clientX: number, clientY: number, target: TestNode, button = 0) {
		this.button = button;
		this.clientX = clientX;
		this.clientY = clientY;
		this.target = target;
	}
}

describe('activateWorkbenchContextMenuOutsideTarget', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('activates the first Workbench tab control beneath the modal context-menu layer', () => {
		const click = vi.fn<() => void>();
		const control = { click };
		const inertLayer = { closest: vi.fn<() => null>(() => null) };
		const tabLabel = { closest: vi.fn<() => typeof control>(() => control) };
		const elementsFromPoint = vi.fn<(clientX: number, clientY: number) => readonly unknown[]>(
			() => [inertLayer, tabLabel],
		);
		vi.stubGlobal('Node', TestNode);
		vi.stubGlobal('MouseEvent', TestMouseEvent);

		activateWorkbenchContextMenuOutsideTarget(
			new TestMouseEvent(650, 17, new TestNode({ elementsFromPoint })) as unknown as Event,
		);

		expect(elementsFromPoint).toHaveBeenCalledWith(650, 17);
		expect(click).toHaveBeenCalledOnce();
	});

	it('does not turn a secondary outside press into a primary Workbench control activation', () => {
		const click = vi.fn<() => void>();
		const control = { click };
		const groupLabel = { closest: vi.fn<() => typeof control>(() => control) };
		const elementsFromPoint = vi.fn<(clientX: number, clientY: number) => readonly unknown[]>(
			() => [groupLabel],
		);
		vi.stubGlobal('Node', TestNode);
		vi.stubGlobal('MouseEvent', TestMouseEvent);

		activateWorkbenchContextMenuOutsideTarget(
			new TestMouseEvent(442, 17, new TestNode({ elementsFromPoint }), 2) as unknown as Event,
		);

		expect(elementsFromPoint).not.toHaveBeenCalled();
		expect(click).not.toHaveBeenCalled();
	});

	it('does not activate unrelated content beneath the context-menu layer', () => {
		const click = vi.fn<() => void>();
		const unrelatedContent = { click, closest: vi.fn<() => null>(() => null) };
		const elementsFromPoint = vi.fn<(clientX: number, clientY: number) => readonly unknown[]>(
			() => [unrelatedContent],
		);
		vi.stubGlobal('Node', TestNode);
		vi.stubGlobal('MouseEvent', TestMouseEvent);

		activateWorkbenchContextMenuOutsideTarget(
			new TestMouseEvent(300, 300, new TestNode({ elementsFromPoint })) as unknown as Event,
		);

		expect(elementsFromPoint).toHaveBeenCalledWith(300, 300);
		expect(click).not.toHaveBeenCalled();
	});
});
