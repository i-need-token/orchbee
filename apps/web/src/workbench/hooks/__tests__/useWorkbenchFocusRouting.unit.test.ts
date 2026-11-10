// @vitest-environment jsdom

import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';
import { act, cleanup, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { registerWorkbenchFocusActions } from '../../actions/registerWorkbenchFocusActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
} from '../../actions/workbench-action-ids';
import type { WorkbenchFocusRouting } from '../../types/workbench-focus-routing';
import { useWorkbenchFocusRouting } from '../useWorkbenchFocusRouting';

function FocusRoutingHarness({
	capture,
	isSidebarVisible = true,
	showCommandSurface = false,
	showDialog = true,
}: {
	readonly capture: (routing: WorkbenchFocusRouting) => void;
	readonly isSidebarVisible?: boolean | undefined;
	readonly showCommandSurface?: boolean | undefined;
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
		showCommandSurface
			? React.createElement(
					'div',
					{ 'data-workbench-command-surface': 'true', role: 'dialog' },
					React.createElement('input', { 'aria-label': 'Search' }),
				)
			: null,
		React.createElement(
			'output',
			{ 'aria-label': 'sidebar request' },
			String(routing.sidebarFocusRequest),
		),
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

	it('lets a Search row move focus once the Search has closed, from where the Search returns it', async () => {
		let routing: WorkbenchFocusRouting | undefined;
		const capture = (candidate: WorkbenchFocusRouting): void => {
			routing = candidate;
		};
		const rendered = render(
			React.createElement(FocusRoutingHarness, { capture, showDialog: false }),
		);
		if (routing === undefined) throw new Error('Expected focus routing to be captured.');
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		registerWorkbenchFocusActions({
			focusRouter: routing,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		});
		const searchRow = () =>
			runtime
				.discover({ scope, source: 'command-surface' })
				.find((result) => result.descriptor.id === WORKBENCH_ACTION_IDS.focusNextRegion);
		const keyboardAvailability = () =>
			runtime.resolveAction(WORKBENCH_ACTION_IDS.focusNextRegion, undefined, {
				scope,
				source: 'keyboard',
			})?.availability;

		// The Search opens over the Tab strip and takes the keyboard.
		act(() => screen.getByRole('button', { name: 'Tab Strip' }).focus());
		rendered.rerender(
			React.createElement(FocusRoutingHarness, {
				capture,
				showCommandSurface: true,
				showDialog: false,
			}),
		);
		act(() => screen.getByRole('textbox', { name: 'Search' }).focus());
		await act(async () => undefined);

		const surfaceOpen = {
			kind: 'disabled',
			reason: workbenchMessages.focusUnavailableSurfaceOpen,
		};
		expect(searchRow()?.availability).toEqual({ kind: 'enabled' });
		expect(keyboardAvailability()).toEqual(surfaceOpen);
		// Another dialog still blocks the Search's rows.
		rendered.rerender(
			React.createElement(FocusRoutingHarness, {
				capture,
				showCommandSurface: true,
				showDialog: true,
			}),
		);
		expect(searchRow()?.availability).toEqual(surfaceOpen);
		rendered.rerender(
			React.createElement(FocusRoutingHarness, {
				capture,
				showCommandSurface: true,
				showDialog: false,
			}),
		);

		const row = searchRow();
		if (row?.kind !== 'ready') throw new Error('Expected a ready Search row.');
		await act(async () => {
			await expect(runtime.executePrepared(row.invocation).completion).resolves.toEqual({
				kind: 'completed',
			});
		});
		// Nothing moves while the Search is still open; after it closes, the region after the Tab
		// strip takes focus.
		expect(screen.getByLabelText('sidebar request').textContent).toBe('0');
		rendered.rerender(React.createElement(FocusRoutingHarness, { capture, showDialog: false }));
		await act(async () => undefined);

		expect(screen.getByLabelText('sidebar request').textContent).toBe('1');
		expect(screen.getByLabelText('tab strip request').textContent).toBe('0');
	});
});
