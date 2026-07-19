import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { TabsDivider } from '../tabs';

describe('TabsDivider', () => {
	it('renders one independent divider pixel', () => {
		const markup = renderToStaticMarkup(createElement(TabsDivider));

		expect(markup).toContain('data-slot="tabs-divider"');
		expect(markup).toContain('w-px');
		expect(markup).toContain('bg-border');
		expect(markup).not.toContain('tabs-divider-leading');
	});
});
