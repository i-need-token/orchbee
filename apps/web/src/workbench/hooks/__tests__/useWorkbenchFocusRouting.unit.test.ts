// @vitest-environment jsdom

import { act, cleanup, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import type { WorkbenchFocusRouting } from '../../types/workbench-focus-routing';
import { useWorkbenchFocusRouting } from '../useWorkbenchFocusRouting';

function FocusRoutingHarness({
	capture,
	isSidebarVisible = true,
	showDialog = true,
}: {
	readonly capture: (routing: WorkbenchFocusRouting) => void;
	readonly isSidebarVisible?: boolean | undefined;
	readonly showDialog?: boolean | undefined;
}): React.ReactNode {
	const routing = useWorkbenchFocusRouting({
		isSidebarVisible,
		resolveTerminalTarget: () => 'terminal-presentation-1',
	});
	capture(routing);
	return React.createElement(
		'div',
		{
			onBlurCapture: routing.onBlurCapture,
			onFocusCapture: routing.onFocusCapture,
		},
		React.createElement(
			'button',
			{ 'data-workbench-focus-region': 'terminal', type: 'button' },
			'Terminal',
		),
		React.createElement(
			'button',
			{ 'data-workbench-focus-region': 'tab-strip', type: 'button' },
			'Tab Strip',
		),
		isSidebarVisible
			? React.createElement(
					'button',
					{ 'data-workbench-focus-region': 'sidebar', type: 'button' },
					'Sidebar',
				)
			: null,
		showDialog ? React.createElement('div', { role: 'dialog', tabIndex: -1 }, 'Dialog') : null,
		React.createElement(
			'output',
			{ 'aria-label': 'terminal autofocus' },
			String(routing.terminalAutoFocusAllowed),
		),
		React.createElement(
			'output',
			{ 'aria-label': 'tab strip request' },
			String(routing.tabStripFocusRequest),
		),
	);
}

afterEach(() => cleanup());

describe('useWorkbenchFocusRouting', () => {
	it('routes semantic requests and suppresses terminal autofocus for other or exclusive owners', () => {
		let routing: WorkbenchFocusRouting | undefined;
		const capture = (candidate: WorkbenchFocusRouting): void => {
			routing = candidate;
		};
		const rendered = render(
			React.createElement(FocusRoutingHarness, {
				capture,
				showDialog: false,
			}),
		);
		if (routing === undefined) throw new Error('Expected focus routing to be captured.');

		const terminal = screen.getByRole('button', { name: 'Terminal' });
		const tabStrip = screen.getByRole('button', { name: 'Tab Strip' });
		terminal.focus();
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('true');

		let resolution: ReturnType<WorkbenchFocusRouting['focus']> | undefined;
		act(() => {
			resolution = routing?.focus({ direction: 'next', kind: 'cycle' });
		});
		expect(resolution).toEqual({ kind: 'target', region: 'tab-strip' });
		expect(screen.getByLabelText('tab strip request').textContent).toBe('1');
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('false');

		tabStrip.focus();
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('false');
		rendered.rerender(
			React.createElement(FocusRoutingHarness, {
				capture,
				showDialog: true,
			}),
		);
		const dialog = screen.getByRole('dialog');
		dialog.focus();
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('false');
		expect(routing.getResolution({ direction: 'next', kind: 'cycle' })).toEqual({
			kind: 'unavailable',
			reason: 'exclusive-surface-active',
		});
		expect(routing.requestTerminalFocus()).toBe(false);

		rendered.rerender(
			React.createElement(FocusRoutingHarness, {
				capture,
				showDialog: false,
			}),
		);
		terminal.focus();
		act(() => {
			expect(routing?.requestTerminalFocus()).toBe(true);
		});
		expect(routing.terminalFocusRequest).toEqual({
			presentationId: 'terminal-presentation-1',
			sequence: 1,
		});
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('true');
	});

	it('retains an exclusive owner while its shell remains and releases it after closure', async () => {
		let routing: WorkbenchFocusRouting | undefined;
		const capture = (candidate: WorkbenchFocusRouting): void => {
			routing = candidate;
		};
		const rendered = render(
			React.createElement(FocusRoutingHarness, {
				capture,
			}),
		);
		if (routing === undefined) throw new Error('Expected focus routing to be captured.');

		const dialog = screen.getByRole('dialog');
		const transientControl = document.createElement('input');
		dialog.append(transientControl);
		act(() => transientControl.focus());
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('false');
		transientControl.remove();
		await act(async () => undefined);

		expect(document.activeElement).toBe(document.body);
		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('false');
		rendered.rerender(
			React.createElement(FocusRoutingHarness, {
				capture,
				showDialog: false,
			}),
		);
		await act(async () => undefined);

		expect(screen.getByLabelText('terminal autofocus').textContent).toBe('true');
	});

	it('hands the terminal the keyboard after the surface that asked has closed', async () => {
		let routing: WorkbenchFocusRouting | undefined;
		const capture = (candidate: WorkbenchFocusRouting): void => {
			routing = candidate;
		};
		const rendered = render(React.createElement(FocusRoutingHarness, { capture }));
		await act(async () => undefined);

		act(() => routing?.beginTerminalHandover().complete());
		expect(routing?.terminalFocusRequest).toBeNull();

		rendered.rerender(React.createElement(FocusRoutingHarness, { capture, showDialog: false }));
		await act(async () => undefined);

		expect(routing?.terminalFocusRequest).toEqual({
			presentationId: 'terminal-presentation-1',
			sequence: 1,
		});
	});

	it.each(['keydown', 'pointerdown'])(
		'lets a later %s end a handover, including one waiting for a surface to close',
		async (press) => {
			let routing: WorkbenchFocusRouting | undefined;
			const capture = (candidate: WorkbenchFocusRouting): void => {
				routing = candidate;
			};
			const rendered = render(React.createElement(FocusRoutingHarness, { capture }));
			await act(async () => undefined);

			const unfinished = routing?.beginTerminalHandover();
			const waiting = routing?.beginTerminalHandover();
			act(() => waiting?.complete());
			document.body.dispatchEvent(new Event(press, { bubbles: true }));
			rendered.rerender(React.createElement(FocusRoutingHarness, { capture, showDialog: false }));
			await act(async () => undefined);
			act(() => unfinished?.complete());

			expect(unfinished?.held()).toBe(false);
			expect(routing?.terminalFocusRequest).toBeNull();
		},
	);
});
