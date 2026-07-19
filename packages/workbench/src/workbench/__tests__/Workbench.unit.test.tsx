import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Workbench } from '../Workbench';

describe('Workbench', () => {
	it('renders only the empty left and right panels', () => {
		const markup = renderToStaticMarkup(createElement(Workbench));

		expect(markup).toContain('data-slot="workbench-left-panel"');
		expect(markup).toContain('data-slot="workbench-right-panel"');
		expect(markup).not.toContain('Orchbee');
		expect(markup).not.toContain('terminal');
	});
});
