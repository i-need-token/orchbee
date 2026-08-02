// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import { DesktopApplicationExitRequestIdSchema } from '../../../../shared';
import type { DesktopApplicationLifetimeState } from '../../model/DesktopApplicationLifetimeCoordinator';
import type { DesktopApplicationLifetimeDialogController } from '../../types/desktop-application-lifetime-dialog';
import { useDesktopApplicationLifetimeDialog } from '../useDesktopApplicationLifetimeDialog';

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
	readonly controller: DesktopApplicationLifetimeDialogController;
}): ReactNode {
	const state = useDesktopApplicationLifetimeDialog(controller);
	return createElement('div', { 'data-lifetime-state': state.kind });
}

describe('useDesktopApplicationLifetimeDialog', () => {
	it('subscribes the dialog to the coordinator external store', async () => {
		let listener: (() => void) | undefined;
		let state: DesktopApplicationLifetimeState = { kind: 'idle' };
		const controller: DesktopApplicationLifetimeDialogController = {
			choose: vi.fn<DesktopApplicationLifetimeDialogController['choose']>(),
			getSnapshot: () => state,
			quitAnyway: vi.fn<DesktopApplicationLifetimeDialogController['quitAnyway']>(),
			retry: vi.fn<DesktopApplicationLifetimeDialogController['retry']>(),
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
		expect(container.firstElementChild?.getAttribute('data-lifetime-state')).toBe('idle');
		await act(async () => {
			state = {
				kind: 'awaiting-choice',
				requestId: DesktopApplicationExitRequestIdSchema.parse('exit-hook'),
				sessionCount: 1,
			};
			listener?.();
		});
		expect(container.firstElementChild?.getAttribute('data-lifetime-state')).toBe(
			'awaiting-choice',
		);
		await act(async () => root.unmount());
	});
});
