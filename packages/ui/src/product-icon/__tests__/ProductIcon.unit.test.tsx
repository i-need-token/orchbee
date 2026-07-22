import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { ProductIcon } from '../ProductIcon';

describe('ProductIcon', () => {
	it.each(['add', 'close', 'pin', 'search', 'settings', 'sidebar-left', 'terminal'] as const)(
		'renders the %s icon as decorative content',
		(name) => {
			const markup = renderToStaticMarkup(createElement(ProductIcon, { name }));

			expect(markup).toContain('aria-hidden="true"');
			expect(markup).toContain(`data-icon="${name}"`);
		},
	);
});
