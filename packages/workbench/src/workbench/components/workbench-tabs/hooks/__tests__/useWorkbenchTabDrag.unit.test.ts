import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { createWorkbenchTabsState } from '../../../../../workbench-tabs';
import { useWorkbenchTabDrag } from '../useWorkbenchTabDrag';

const TABS = [
	{ id: 'first', label: 'First' },
	{ id: 'second', label: 'Second' },
] as const;

const STATE = createWorkbenchTabsState({ tabs: TABS });

function WorkbenchTabDragHarness({ enabled }: { readonly enabled: boolean }) {
	const tabDrag = useWorkbenchTabDrag(STATE, enabled ? () => undefined : undefined, undefined);
	const properties = tabDrag.getTabDragProperties('first');

	return createElement('div', {
		'data-reorderable': properties.reorderable ? true : undefined,
	});
}

function WorkbenchTabGroupDragHarness({ enabled }: { readonly enabled: boolean }) {
	const tabDrag = useWorkbenchTabDrag(STATE, undefined, enabled ? () => undefined : undefined);
	const properties = tabDrag.getTabGroupDragProperties('group-a');

	return createElement('div', {
		'data-reorderable': properties.reorderable ? true : undefined,
	});
}

describe('useWorkbenchTabDrag', () => {
	it('enables pointer reordering when the caller accepts tab moves', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchTabDragHarness, { enabled: true }));

		expect(markup).toContain('data-reorderable="true"');
	});

	it('keeps tabs fixed without an authoritative move action', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchTabDragHarness, { enabled: false }));

		expect(markup).not.toContain('data-reorderable');
	});

	it('enables pointer reordering when the caller accepts group moves', () => {
		const markup = renderToStaticMarkup(
			createElement(WorkbenchTabGroupDragHarness, { enabled: true }),
		);

		expect(markup).toContain('data-reorderable="true"');
	});
});
