import { createElement, useRef } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchTabLayout } from '../useWorkbenchTabLayout';

const TABS = [{ id: 'first', label: 'First' }] as const;

function WorkbenchTabLayoutHarness() {
	const tabListRef = useRef<HTMLDivElement>(null);
	const tabLayout = useWorkbenchTabLayout(TABS, 0, tabListRef);
	const properties = tabLayout.getTabLayoutProperties('first');
	return createElement('div', {
		'data-width-locked': properties['data-workbench-tab-width-locked'],
	});
}

describe('useWorkbenchTabLayout', () => {
	it('starts in fluid layout during server rendering', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchTabLayoutHarness));

		expect(markup).toBe('<div></div>');
	});
});
