import { describe, expect, it, vi } from 'vitest';

import { createPanelDragSnapshot } from '../createPanelDragSnapshot';

describe('createPanelDragSnapshot', () => {
	it('sanitizes and mounts an element clone', () => {
		const removeAttribute = vi.fn<(name: string) => void>();
		const setAttribute = vi.fn<(name: string, value: string) => void>();
		const snapshotElement = {
			nodeType: 1,
			querySelector: vi.fn<(selector: string) => Element | null>(() => null),
			querySelectorAll: vi.fn<(selector: string) => NodeListOf<Element>>(
				() => [] as unknown as NodeListOf<Element>,
			),
			removeAttribute,
			setAttribute,
		} as unknown as HTMLElement;
		const sourceElement = {
			cloneNode: vi.fn<(deep?: boolean) => Node>(() => snapshotElement),
			querySelectorAll: vi.fn<(selector: string) => NodeListOf<HTMLCanvasElement>>(
				() => [] as unknown as NodeListOf<HTMLCanvasElement>,
			),
		} as unknown as HTMLElement;
		const snapshot = createPanelDragSnapshot(sourceElement);
		const replaceChildren = vi.fn<(...nodes: Node[]) => void>();
		const host = {
			firstElementChild: null,
			replaceChildren,
		} as unknown as HTMLDivElement;

		snapshot?.mount(host);

		expect(sourceElement.cloneNode).toHaveBeenCalledWith(true);
		expect(removeAttribute).toHaveBeenCalledWith('data-panel-id');
		expect(setAttribute).toHaveBeenCalledWith('inert', '');
		expect(setAttribute).toHaveBeenCalledWith('data-panel-drag-snapshot-source', 'true');
		expect(replaceChildren).toHaveBeenCalledWith(snapshotElement);
	});

	it('rejects a non-element clone', () => {
		const sourceElement = {
			cloneNode: vi.fn<(deep?: boolean) => Node>(() => ({ nodeType: 3 }) as Node),
		} as unknown as HTMLElement;

		expect(createPanelDragSnapshot(sourceElement)).toBeNull();
	});
});
