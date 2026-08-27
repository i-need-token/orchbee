// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	BrowserAuthenticationControllerPort,
	BrowserAuthenticationState,
} from '../../model/browser-authentication-controller';
import { useBrowserAuthentication } from '../useBrowserAuthentication';

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
	readonly controller: BrowserAuthenticationControllerPort;
}): ReactNode {
	const state = useBrowserAuthentication(controller);
	return createElement('div', { 'data-authentication-error': state.error ?? 'none' });
}

describe('useBrowserAuthentication', () => {
	it('subscribes the sign-in page to the authentication controller', async () => {
		let listener: (() => void) | undefined;
		let state: BrowserAuthenticationState = {
			error: null,
			pending: false,
			personalAccessToken: '',
		};
		const controller: BrowserAuthenticationControllerPort = {
			dispose: vi.fn<BrowserAuthenticationControllerPort['dispose']>(),
			getSnapshot: () => state,
			setPersonalAccessToken:
				vi.fn<BrowserAuthenticationControllerPort['setPersonalAccessToken']>(),
			submit: vi.fn<BrowserAuthenticationControllerPort['submit']>(),
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
		expect(container.firstElementChild?.getAttribute('data-authentication-error')).toBe('none');
		await act(async () => {
			state = { ...state, error: 'invalid-personal-access-token' };
			listener?.();
		});
		expect(container.firstElementChild?.getAttribute('data-authentication-error')).toBe(
			'invalid-personal-access-token',
		);
		await act(async () => root.unmount());
	});
});
