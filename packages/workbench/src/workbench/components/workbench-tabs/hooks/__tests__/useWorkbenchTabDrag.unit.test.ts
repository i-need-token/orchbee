import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchTabDrag } from '../useWorkbenchTabDrag';

const TABS = [
	{ id: 'first', label: 'First' },
	{ id: 'second', label: 'Second' },
] as const;

function WorkbenchTabDragHarness({ enabled }: { readonly enabled: boolean }) {
	const tabDrag = useWorkbenchTabDrag(TABS, enabled ? () => undefined : undefined, 'drag-preview');
	const properties = tabDrag.getTabDragProperties('first');

	return createElement('div', { draggable: properties.draggable });
}

describe('useWorkbenchTabDrag', () => {
	it('enables native dragging when the caller accepts tab moves', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchTabDragHarness, { enabled: true }));

		expect(markup).toContain('draggable="true"');
	});

	it('keeps tabs non-draggable without an authoritative move action', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchTabDragHarness, { enabled: false }));

		expect(markup).toContain('draggable="false"');
	});
});
