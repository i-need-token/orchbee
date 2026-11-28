// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, render, screen } from '@testing-library/react';
import { createRef, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { FileOverlayRegistry } from '../../../../workspace-stores';
import type { FileOverlayCommands, FileOverlayFocus } from '../../../file-overlay-contract';
import { FileOverlayFrame } from '../components/file-overlay-frame/FileOverlayFrame';
import { FileOverlayHostStoreProvider } from '../contexts/file-overlay-host-store';
import { useFileOverlayFocusRequest } from '../hooks/useFileOverlayFocusRequest';

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

const NO_COMMANDS: FileOverlayCommands = {
	closeTab: () => undefined,
	keepTab: () => undefined,
	selectTab: () => undefined,
	tabMenu: () => [],
	titleActions: () => ({ buttons: [], more: [] }),
	toggleFull: () => undefined,
	unpinTab: () => undefined,
};

/** The overlay's frame with a tab for each path, and the handle the Workbench asks for the keys by. */
function renderOverlay(paths: readonly string[]) {
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
	}
	const reference = createRef<FileOverlayFocus>();
	function Overlay(): ReactNode {
		const focusRequest = useFileOverlayFocusRequest(reference);
		return (
			<FileOverlayHostStoreProvider
				commands={NO_COMMANDS}
				dispatch={(event) => {
					store.dispatch(event);
				}}
				focusRequest={focusRequest}
				focusTerminal={() => undefined}
				platform='standard'
				shortcutLabels={{ toggleFull: undefined }}
				store={store}
			>
				<FileOverlayFrame />
			</FileOverlayHostStoreProvider>
		);
	}
	render(
		<OrchbeeI18nProvider locale='en-US'>
			<Overlay />
		</OrchbeeI18nProvider>,
	);
	const tabId = (path: string): string =>
		store.state.getState().tabs.tabs.find((tab) => tab.input.resource.path === path)?.id ?? '';
	return { reference, store, tabId };
}

describe('the file overlay focus request', () => {
	it('gives the keys to the tab an ask follows once the row shows it, and only for that ask', () => {
		const { reference, store, tabId } = renderOverlay(['/a.ts', '/b.ts', '/c.ts']);
		act(() => {
			store.dispatch({ id: tabId('/a.ts'), type: 'tab-activated' });
			reference.current?.focus();
		});
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'a.ts' }));
		const elsewhere = document.body.appendChild(document.createElement('button'));
		elsewhere.focus();
		act(() => {
			store.dispatch({ id: tabId('/b.ts'), type: 'tab-activated' });
		});
		expect(document.activeElement).toBe(elsewhere);
		elsewhere.remove();
	});
});
