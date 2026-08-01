// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { createElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchConfirmationAnchor } from '../../types/workbench-confirmation';
import { workbenchConfirmationAnchorFromMenuEvent } from '../workbenchConfirmationAnchorFromMenuEvent';

afterEach(cleanup);

describe('workbenchConfirmationAnchorFromMenuEvent', () => {
	it('uses pointer coordinates for pointer menu selection', () => {
		let anchor: WorkbenchConfirmationAnchor | null = null;
		render(
			createElement(
				'button',
				{
					onClick: (event) => {
						anchor = workbenchConfirmationAnchorFromMenuEvent(event);
					},
				},
				'Delete',
			),
		);

		fireEvent.click(screen.getByRole('button'), { clientX: 320, clientY: 240, detail: 1 });

		expect(anchor).toEqual({ x: 320, y: 240 });
	});

	it('uses the menu item edge for keyboard selection', () => {
		let anchor: WorkbenchConfirmationAnchor | null = null;
		render(
			createElement(
				'button',
				{
					onClick: (event) => {
						anchor = workbenchConfirmationAnchorFromMenuEvent(event);
					},
				},
				'Delete',
			),
		);
		const button = screen.getByRole('button');
		vi.spyOn(button, 'getBoundingClientRect').mockReturnValue(new DOMRect(100, 200, 40, 20));

		fireEvent.click(button, { detail: 0 });

		expect(anchor).toEqual({ x: 140, y: 210 });
	});
});
