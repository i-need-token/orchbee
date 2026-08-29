// @vitest-environment jsdom

import { cleanup, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { useWorkbenchSidebarFocusRequest } from '../useWorkbenchSidebarFocusRequest';

function FocusHarness({
	focusRequest,
	showCurrent,
}: {
	readonly focusRequest: number;
	readonly showCurrent: boolean;
}): React.ReactNode {
	const rootReference = useWorkbenchSidebarFocusRequest(focusRequest);
	return React.createElement(
		'aside',
		{ ref: rootReference },
		React.createElement(
			'button',
			{ 'data-slot': 'workbench-sidebar-terminal', type: 'button' },
			'Terminal',
		),
		showCurrent
			? React.createElement(
					'button',
					{ 'aria-current': 'page', type: 'button' },
					'Current workspace',
				)
			: null,
	);
}

afterEach(() => cleanup());

describe('useWorkbenchSidebarFocusRequest', () => {
	it('focuses the current location and falls back to the standalone terminal', () => {
		const rendered = render(
			React.createElement(FocusHarness, { focusRequest: 1, showCurrent: true }),
		);
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Current workspace' }));

		rendered.rerender(React.createElement(FocusHarness, { focusRequest: 2, showCurrent: false }));
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Terminal' }));
	});
});
