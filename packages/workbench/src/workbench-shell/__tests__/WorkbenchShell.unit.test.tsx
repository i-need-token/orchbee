import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WorkbenchShell } from '../WorkbenchShell';

describe('WorkbenchShell', () => {
	it('renders two empty resizable panels', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchShell));

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).toContain('data-slot="resizable-panel-group"');
		expect(markup).toContain('data-slot="resizable-handle"');
		expect(markup).toContain('role="separator"');
		expect(markup).toContain('flex-basis:250px');
	});
});
