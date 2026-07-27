import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { ProductIcon } from '../ProductIcon';

describe('ProductIcon', () => {
	it.each([
		'action',
		'add',
		'agent',
		'arrow-left',
		'close',
		'ellipsis',
		'layout',
		'more-vertical',
		'panels',
		'pin',
		'search',
		'settings',
		'sidebar-left',
		'tabs',
		'terminal',
	] as const)('renders the %s icon as decorative content', (name) => {
		const markup = renderToStaticMarkup(createElement(ProductIcon, { name }));

		expect(markup).toContain('aria-hidden="true"');
		expect(markup).toContain(`data-icon="${name}"`);
	});
});
