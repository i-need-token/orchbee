// @vitest-environment jsdom

import { cleanup, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { useWorkbenchSidebarFocusRequest } from '../useWorkbenchSidebarFocusRequest';

function FocusHarness({
	activeProject = false,
	focusRequest,
	onActivateLocation,
	onToggleProject,
	showCurrent,
}: {
	readonly activeProject?: boolean;
	readonly focusRequest: number;
	readonly onActivateLocation?: () => void;
	readonly onToggleProject?: () => void;
	readonly showCurrent: boolean;
}): React.ReactNode {
	const rootReference = useWorkbenchSidebarFocusRequest(focusRequest);
	return React.createElement(
		React.Fragment,
		null,
		React.createElement('button', { type: 'button' }, 'Other surface'),
		React.createElement(
			'aside',
			{ 'aria-label': 'Sidebar', ref: rootReference },
			React.createElement(
				'section',
				{ 'aria-label': 'Projects', 'data-navigation-reorder-scroll': 'true' },
				React.createElement(
					'button',
					{
						'aria-expanded': showCurrent,
						'data-holds-active-workspace': activeProject ? '' : undefined,
						'data-slot': 'workbench-sidebar-project',
						onClick: onToggleProject,
						type: 'button',
					},
					'Project',
				),
				showCurrent
					? React.createElement(
							'button',
							{ 'aria-current': 'page', onClick: onActivateLocation, type: 'button' },
							'Current workspace',
						)
					: null,
			),
		),
	);
}

function setScrollGeometry(owner: HTMLElement, target: HTMLElement, top: number, height: number) {
	Object.defineProperties(owner, {
		clientHeight: { configurable: true, value: 100 },
		clientTop: { configurable: true, value: 2 },
		scrollHeight: { configurable: true, value: 600 },
	});
	vi.spyOn(owner, 'getBoundingClientRect').mockReturnValue(new DOMRect(0, 100, 200, 104));
	return vi
		.spyOn(target, 'getBoundingClientRect')
		.mockReturnValue(new DOMRect(0, top, 200, height));
}

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('useWorkbenchSidebarFocusRequest', () => {
	it('prefers the current Workspace, then its collapsed Project, without navigation', () => {
		const onActivateLocation = vi.fn<() => void>();
		const onToggleProject = vi.fn<() => void>();
		const callbacks = { onActivateLocation, onToggleProject };
		const rendered = render(
			React.createElement(FocusHarness, {
				...callbacks,
				activeProject: true,
				focusRequest: 1,
				showCurrent: true,
			}),
		);
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Current workspace' }));
		const project = screen.getByRole('button', { name: 'Project' });
		expect(project.getAttribute('aria-expanded')).toBe('true');

		rendered.rerender(
			React.createElement(FocusHarness, {
				...callbacks,
				activeProject: true,
				focusRequest: 2,
				showCurrent: false,
			}),
		);
		expect(document.activeElement).toBe(project);
		expect(project.getAttribute('aria-expanded')).toBe('false');
		expect(project.hasAttribute('data-holds-active-workspace')).toBe(true);
		expect(screen.queryByRole('button', { name: 'Current workspace' })).toBeNull();
		expect(onActivateLocation).not.toHaveBeenCalled();
		expect(onToggleProject).not.toHaveBeenCalled();
	});

	it('leaves focus on another surface until a new positive request finds a destination', () => {
		const rendered = render(
			React.createElement(FocusHarness, { focusRequest: 0, showCurrent: true }),
		);
		const otherSurface = screen.getByRole('button', { name: 'Other surface' });
		otherSurface.focus();
		rendered.rerender(
			React.createElement(FocusHarness, {
				activeProject: true,
				focusRequest: 0,
				showCurrent: false,
			}),
		);
		expect(document.activeElement).toBe(otherSurface);

		rendered.rerender(React.createElement(FocusHarness, { focusRequest: 1, showCurrent: true }));
		expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Current workspace' }));
		otherSurface.focus();
		rendered.rerender(React.createElement(FocusHarness, { focusRequest: 1, showCurrent: false }));
		expect(document.activeElement).toBe(otherSurface);

		rendered.rerender(React.createElement(FocusHarness, { focusRequest: 2, showCurrent: false }));
		expect(document.activeElement).toBe(otherSurface);
	});

	it.each([
		{ name: 'above the viewport', top: 92, height: 20, initial: 120, expected: 110 },
		{ name: 'below the viewport', top: 192, height: 30, initial: 120, expected: 140 },
		{ name: 'fully visible', top: 112, height: 80, initial: 120, expected: 120 },
		{ name: 'exactly fills the viewport', top: 102, height: 100, initial: 120, expected: 120 },
		{ name: 'oversized and spans the viewport', top: 82, height: 140, initial: 120, expected: 120 },
		{ name: 'oversized below the viewport', top: 112, height: 120, initial: 120, expected: 130 },
		{ name: 'oversized above the viewport', top: 72, height: 120, initial: 120, expected: 110 },
		{ name: 'clamps to the start', top: 82, height: 10, initial: 5, expected: 0 },
		{ name: 'clamps to the end', top: 212, height: 20, initial: 490, expected: 500 },
	])(
		'reveals a row $name only in its Project scroll region',
		({ top, height, initial, expected }) => {
			const rendered = render(
				React.createElement(FocusHarness, { focusRequest: 0, showCurrent: true }),
			);
			const sidebar = screen.getByRole('complementary', { name: 'Sidebar' });
			const owner = screen.getByRole('region', { name: 'Projects' });
			const target = screen.getByRole('button', { name: 'Current workspace' });
			setScrollGeometry(owner, target, top, height);
			owner.scrollTop = initial;
			sidebar.scrollTop = 17;
			const focus = vi.spyOn(target, 'focus');

			rendered.rerender(React.createElement(FocusHarness, { focusRequest: 1, showCurrent: true }));
			expect(document.activeElement).toBe(target);
			expect(focus).toHaveBeenCalledExactlyOnceWith({ preventScroll: true });
			expect(owner.scrollTop).toBe(expected);
			expect(sidebar.scrollTop).toBe(17);
		},
	);

	it('reveals the collapsed Project again on a new request even when it already has focus', () => {
		const rendered = render(
			React.createElement(FocusHarness, {
				activeProject: true,
				focusRequest: 0,
				showCurrent: false,
			}),
		);
		const owner = screen.getByRole('region', { name: 'Projects' });
		const target = screen.getByRole('button', { name: 'Project' });
		const bounds = setScrollGeometry(owner, target, 192, 30);
		owner.scrollTop = 120;
		rendered.rerender(
			React.createElement(FocusHarness, {
				activeProject: true,
				focusRequest: 1,
				showCurrent: false,
			}),
		);
		expect(document.activeElement).toBe(target);
		expect(owner.scrollTop).toBe(140);

		bounds.mockReturnValue(new DOMRect(0, 82, 200, 20));
		rendered.rerender(
			React.createElement(FocusHarness, {
				activeProject: true,
				focusRequest: 1,
				showCurrent: false,
			}),
		);
		expect(owner.scrollTop).toBe(140);
		rendered.rerender(
			React.createElement(FocusHarness, {
				activeProject: true,
				focusRequest: 2,
				showCurrent: false,
			}),
		);
		expect(document.activeElement).toBe(target);
		expect(owner.scrollTop).toBe(120);
		expect(target.getAttribute('aria-expanded')).toBe('false');
	});

	it('does not scroll zero-height geometry', () => {
		const rendered = render(
			React.createElement(FocusHarness, {
				focusRequest: 0,
				showCurrent: true,
			}),
		);
		const owner = screen.getByRole('region', { name: 'Projects' });
		const target = screen.getByRole('button', { name: 'Current workspace' });
		const bounds = setScrollGeometry(owner, target, 82, 0);
		owner.scrollTop = 120;
		rendered.rerender(
			React.createElement(FocusHarness, {
				focusRequest: 1,
				showCurrent: true,
			}),
		);
		expect(document.activeElement).toBe(target);
		expect(owner.scrollTop).toBe(120);
		bounds.mockReturnValue(new DOMRect(0, 82, 200, 20));
		Object.defineProperty(owner, 'clientHeight', { value: 0 });
		rendered.rerender(
			React.createElement(FocusHarness, {
				focusRequest: 2,
				showCurrent: true,
			}),
		);
		expect(owner.scrollTop).toBe(120);
	});
});
