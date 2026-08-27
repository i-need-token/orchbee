// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	DesktopBrowserAccessDialogController,
	DesktopBrowserAccessDialogState,
} from '../../model/DesktopBrowserAccessController';
import { useDesktopBrowserAccessDialog } from '../useDesktopBrowserAccessDialog';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

function TestHarness({
	controller,
}: {
	readonly controller: DesktopBrowserAccessDialogController;
}): ReactNode {
	const state = useDesktopBrowserAccessDialog(controller);
	return createElement('div', { 'data-browser-access-phase': state.snapshot.access.phase });
}

describe('useDesktopBrowserAccessDialog', () => {
	it('subscribes the dialog to the Browser-access external store', async () => {
		let listener: (() => void) | undefined;
		let state: DesktopBrowserAccessDialogState = {
			copiedOrigin: null,
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: null,
			open: true,
			pendingAction: null,
			resetConfirmationOpen: false,
			snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'configured' },
		};
		const controller: DesktopBrowserAccessDialogController = {
			cancelPersonalAccessTokenReset:
				vi.fn<DesktopBrowserAccessDialogController['cancelPersonalAccessTokenReset']>(),
			close: vi.fn<DesktopBrowserAccessDialogController['close']>(),
			confirmPersonalAccessTokenReset:
				vi.fn<DesktopBrowserAccessDialogController['confirmPersonalAccessTokenReset']>(),
			copyAccessLink: vi.fn<DesktopBrowserAccessDialogController['copyAccessLink']>(),
			copyPersonalAccessToken:
				vi.fn<DesktopBrowserAccessDialogController['copyPersonalAccessToken']>(),
			dispose: vi.fn<DesktopBrowserAccessDialogController['dispose']>(),
			getSnapshot: () => state,
			open: vi.fn<DesktopBrowserAccessDialogController['open']>(),
			openLocalBrowser: vi.fn<DesktopBrowserAccessDialogController['openLocalBrowser']>(),
			requestPersonalAccessTokenIssue:
				vi.fn<DesktopBrowserAccessDialogController['requestPersonalAccessTokenIssue']>(),
			start: vi.fn<DesktopBrowserAccessDialogController['start']>(),
			stop: vi.fn<DesktopBrowserAccessDialogController['stop']>(),
			subscribe: (nextListener) => {
				listener = nextListener;
				return () => {
					listener = undefined;
				};
			},
		};
		const container = document.createElement('div');
		const root = createRoot(container);

		await act(async () => root.render(createElement(TestHarness, { controller })));
		expect(container.firstElementChild?.getAttribute('data-browser-access-phase')).toBe('stopped');
		await act(async () => {
			state = {
				...state,
				snapshot: {
					access: {
						localOrigin: 'http://127.0.0.1:43110',
						mode: 'loopback',
						networkOrigins: [],
						phase: 'ready',
					},
					personalAccessToken: 'configured',
				},
			};
			listener?.();
		});
		expect(container.firstElementChild?.getAttribute('data-browser-access-phase')).toBe('ready');
		await act(async () => root.unmount());
	});
});
