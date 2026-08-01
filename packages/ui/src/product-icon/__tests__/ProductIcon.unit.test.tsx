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
		'clipboard',
		'chevron-right',
		'check',
		'close',
		'copy',
		'edit',
		'ellipsis',
		'folder',
		'folder-opened',
		'git-branch',
		'layout',
		'more-vertical',
		'new-folder',
		'panels',
		'pin',
		'search',
		'screen-full',
		'screen-normal',
		'settings',
		'sidebar-left',
		'tabs',
		'terminal',
		'trash',
	] as const)('renders the %s icon as decorative content', (name) => {
		const markup = renderToStaticMarkup(createElement(ProductIcon, { name }));

		expect(markup).toContain('aria-hidden="true"');
		expect(markup).toContain(`data-icon="${name}"`);
	});

	it.each(['menu-leading', 'section-action'] as const)(
		'exposes the %s size as an explicit component variant',
		(size) => {
			const markup = renderToStaticMarkup(createElement(ProductIcon, { name: 'add', size }));

			expect(markup).toContain(`data-icon-size="${size}"`);
		},
	);
});
