import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { WorkbenchPanelDragGhost } from '../WorkbenchPanelDragGhost';

describe('WorkbenchPanelDragGhost', () => {
	it('mounts a prepared DOM snapshot without fabricating panel content', () => {
		const markup = renderToStaticMarkup(
			createElement(WorkbenchPanelDragGhost, {
				ghost: {
					blockStart: 40,
					height: 192,
					inlineStart: 80,
					mountSnapshot: () => undefined,
					scale: 0.5,
					sourceHeight: 800,
					sourceWidth: 600,
					width: 300,
				},
			}),
		);

		expect(markup).toContain('data-slot="workbench-panel-drag-ghost"');
		expect(markup).toContain('data-slot="workbench-panel-drag-snapshot"');
		expect(markup).toContain('--ob-comp-workbench-panel-drag-scale:0.5');
		expect(markup).not.toContain('Terminal 1');
	});
});
