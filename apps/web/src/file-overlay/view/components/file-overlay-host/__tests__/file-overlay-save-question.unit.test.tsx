// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { createRef, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { FileTab } from '../../../../model';
import {
	dispatchFileOverlayEvent,
	FileOverlayRegistry,
	type FileTabContentPort,
} from '../../../../workspace-stores';
import type {
	FileOverlayCommands,
	FileOverlayEditorHost,
	FileOverlayFocus,
} from '../../../file-overlay-contract';
import { FileOverlayCloseDialog } from '../components/file-overlay-close-dialog/FileOverlayCloseDialog';
import { FileOverlayFrame } from '../components/file-overlay-frame/FileOverlayFrame';
import { FileOverlayHostStoreProvider } from '../contexts/file-overlay-host-store';
import { useFileOverlayFocusRequest } from '../hooks/useFileOverlayFocusRequest';

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

const NO_COMMANDS: FileOverlayCommands = {
	closeOverlay: () => undefined,
	closeTab: () => undefined,
	keepTab: () => undefined,
	moveTab: () => undefined,
	selectTab: () => undefined,
	tabMenu: () => [],
	titleActions: () => ({ buttons: [], more: [] }),
	toggleFull: () => undefined,
	unpinTab: () => undefined,
};

const actionRuntime = new ActionRuntime();
const NO_EDITOR: FileOverlayEditorHost = {
	actionRuntime,
	actionScope: actionRuntime.createScope('workbench'),
	inFocusRegion: () => false,
	searchActions: () => undefined,
	searchActionsShortcut: undefined,
};

/** The overlay with a tab for each path, those in `unsaved` with unsaved edits, and its save question. */
function renderOverlay(paths: readonly string[], unsaved: readonly string[]) {
	class TestResizeObserver {
		public disconnect(): void {}
		public observe(_target: Element): void {}
		public unobserve(_target: Element): void {}
	}
	vi.stubGlobal('ResizeObserver', TestResizeObserver);
	const store = new FileOverlayRegistry(true).store('workspace-1');
	for (const path of paths) {
		store.dispatch({
			input: { kind: 'file', resource: { key: path, path } },
			preview: false,
			type: 'file-opened',
		});
		if (unsaved.includes(path)) {
			store.dispatch({ id: `file:${path}`, type: 'unsaved-changed', unsaved: true });
		}
	}
	const content = {
		closed: vi.fn<(tabs: readonly FileTab[]) => void>(),
		revert: vi.fn<(tabs: readonly FileTab[]) => void>(),
		save: vi.fn<FileTabContentPort['save']>(() => Promise.resolve([])),
	};
	const returnFocus = vi.fn<() => void>();
	const ports = { content, focusTerminal: () => undefined };
	const reference = createRef<FileOverlayFocus>();
	function Overlay(): ReactNode {
		const focusRequest = useFileOverlayFocusRequest(reference);
		return (
			<FileOverlayHostStoreProvider
				commands={NO_COMMANDS}
				contents={null}
				editor={NO_EDITOR}
				dispatch={(event) => {
					dispatchFileOverlayEvent(store, event, ports);
				}}
				focusRequest={focusRequest}
				focusTerminal={() => undefined}
				platform='standard'
				returnFocus={returnFocus}
				shortcutLabels={{ closeOverlay: undefined, toggleFull: undefined }}
				store={store}
			>
				<FileOverlayFrame pip={null} />
				<FileOverlayCloseDialog />
			</FileOverlayHostStoreProvider>
		);
	}
	render(
		<OrchbeeI18nProvider locale='en-US'>
			<Overlay />
		</OrchbeeI18nProvider>,
	);
	const requestClose = (
		target: Extract<Parameters<typeof store.dispatch>[0], { type: 'close-requested' }>['target'],
	): void => {
		act(() => {
			dispatchFileOverlayEvent(store, { target, type: 'close-requested' }, ports);
		});
	};
	const openIds = (): readonly string[] => store.state.getState().tabs.tabs.map((tab) => tab.id);
	return { content, openIds, requestClose, returnFocus, store };
}

describe('the file overlay save question', () => {
	it('asks about an unsaved tab with Save focused, and Esc keeps it, giving its tab the keys', async () => {
		const { content, openIds, requestClose, store } = renderOverlay(
			['/w/a.ts', '/w/b.ts'],
			['/w/a.ts'],
		);

		requestClose({ ids: ['file:/w/a.ts', 'file:/w/b.ts'], kind: 'tabs' });

		const question = await screen.findByRole('alertdialog', {
			name: 'Do you want to save the changes you made to a.ts?',
		});
		await waitFor(() =>
			expect(document.activeElement).toBe(within(question).getByRole('button', { name: 'Save' })),
		);
		fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });

		await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
		expect(openIds()).toEqual(['file:/w/a.ts', 'file:/w/b.ts']);
		expect(store.state.getState().tabs.pendingClose).toBeNull();
		expect(content.save).not.toHaveBeenCalled();
		// The question showed the tab it asked about, which keeps the keys after it.
		await waitFor(() =>
			expect(document.activeElement).toBe(screen.getByRole('tab', { name: /^a\.ts/u })),
		);
	});

	it("names ten of many unsaved files, and Don't Save drops their edits and gives the keys back", async () => {
		const paths = Array.from({ length: 12 }, (_, index) => `/w/file-${index + 1}.ts`);
		const { content, openIds, requestClose, returnFocus, store } = renderOverlay(paths, paths);

		requestClose({ kind: 'overlay' });

		const question = await screen.findByRole('alertdialog', {
			name: 'Do you want to save the changes to the following 12 files?',
		});
		expect(within(question).getByText('file-10.ts')).toBeDefined();
		expect(within(question).queryByText('file-11.ts')).toBeNull();
		expect(within(question).getByText('…2 additional files not shown')).toBeDefined();
		await waitFor(() =>
			expect(document.activeElement).toBe(
				within(question).getByRole('button', { name: 'Save All' }),
			),
		);
		fireEvent.click(within(question).getByRole('button', { name: 'Don’t Save' }));

		await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
		expect(content.revert.mock.calls.map(([tabs]) => tabs.length)).toEqual([12]);
		expect(content.closed.mock.calls.map(([tabs]) => tabs.length)).toEqual([12]);
		expect(openIds()).toEqual([]);
		expect(store.state.getState().mode.kind).toBe('closed');
		await waitFor(() => expect(returnFocus).toHaveBeenCalledOnce());
	});
});
