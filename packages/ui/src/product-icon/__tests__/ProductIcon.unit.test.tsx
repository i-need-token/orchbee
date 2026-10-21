import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { ProductIcon } from '../ProductIcon';

describe('ProductIcon', () => {
	it('hides decorative icons from the accessibility tree', () => {
		const markup = renderToStaticMarkup(createElement(ProductIcon, { name: 'add' }));

		expect(markup).toContain('aria-hidden="true"');
	});
});
