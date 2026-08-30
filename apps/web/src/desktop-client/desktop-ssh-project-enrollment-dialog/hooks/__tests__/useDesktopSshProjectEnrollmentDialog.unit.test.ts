// @vitest-environment jsdom

import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	DesktopSshProjectEnrollmentDialogController,
	DesktopSshProjectEnrollmentDialogState,
} from '../../model/DesktopSshProjectEnrollmentController';
import { useDesktopSshProjectEnrollmentDialog } from '../useDesktopSshProjectEnrollmentDialog';

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
	readonly controller: DesktopSshProjectEnrollmentDialogController;
}): ReactNode {
	const { state } = useDesktopSshProjectEnrollmentDialog(controller);
	return createElement('div', {
		'data-dialog-phase': state.kind === 'closed' ? 'closed' : state.phase,
	});
}

describe('useDesktopSshProjectEnrollmentDialog', () => {
	it('subscribes the dialog to the enrollment controller external store', async () => {
		let listener: (() => void) | undefined;
		let state: DesktopSshProjectEnrollmentDialogState = { kind: 'closed' };
		const destinationCatalog = {
			discoveryIssue: null,
			managed: [],
			registryIssue: null,
			suggestions: [],
		};
		const controller: DesktopSshProjectEnrollmentDialogController = {
			cancel: vi.fn<DesktopSshProjectEnrollmentDialogController['cancel']>(),
			close: vi.fn<DesktopSshProjectEnrollmentDialogController['close']>(),
			getDestinationCatalogSnapshot: () => destinationCatalog,
			getDialogSnapshot: () => state,
			navigateDirectory: vi.fn<DesktopSshProjectEnrollmentDialogController['navigateDirectory']>(),
			openFolderAttachment:
				vi.fn<DesktopSshProjectEnrollmentDialogController['openFolderAttachment']>(),
			resolvePrompt: vi.fn<DesktopSshProjectEnrollmentDialogController['resolvePrompt']>(),
			retry: vi.fn<DesktopSshProjectEnrollmentDialogController['retry']>(),
			selectDirectory: vi.fn<DesktopSshProjectEnrollmentDialogController['selectDirectory']>(),
			submit: vi.fn<DesktopSshProjectEnrollmentDialogController['submit']>(),
			subscribeDestinationCatalog: () => () => undefined,
			subscribeDialog: (nextListener) => {
				listener = nextListener;
				return () => {
					listener = undefined;
				};
			},
			updateDirectoryPath:
				vi.fn<DesktopSshProjectEnrollmentDialogController['updateDirectoryPath']>(),
			updateForm: vi.fn<DesktopSshProjectEnrollmentDialogController['updateForm']>(),
		};
		const container = document.createElement('div');
		const root = createRoot(container);

		await act(async () => root.render(createElement(TestHarness, { controller })));
		expect(container.firstElementChild?.getAttribute('data-dialog-phase')).toBe('closed');
		await act(async () => {
			state = {
				canNavigate: false,
				directory: null,
				failure: null,
				kind: 'connection',
				pathInput: '',
				phase: 'working',
				progress: null,
				projectId: 'project-remote',
				prompt: null,
			};
			listener?.();
		});
		expect(container.firstElementChild?.getAttribute('data-dialog-phase')).toBe('working');
		await act(async () => root.unmount());
	});
});
