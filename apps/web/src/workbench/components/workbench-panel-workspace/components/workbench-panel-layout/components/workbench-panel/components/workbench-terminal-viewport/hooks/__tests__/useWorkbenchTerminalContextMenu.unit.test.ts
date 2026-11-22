// @vitest-environment jsdom

import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { PanelSplitDirection } from '../../../../../../../../model/panel-workspace-state';
import { useWorkbenchTerminalContextMenu } from '../useWorkbenchTerminalContextMenu';

type ContextMenuControls = ReturnType<typeof useWorkbenchTerminalContextMenu>;

let controls: ContextMenuControls | null = null;

function HookProjection({
	splitPanel,
}: {
	readonly splitPanel: (panelId: string, direction: PanelSplitDirection) => string | null;
}) {
	controls = useWorkbenchTerminalContextMenu({ panelId: 'panel-1', splitPanel });
	return createElement('div');
}

function getControls(): ContextMenuControls {
	if (controls === null) {
		throw new Error('Expected the context menu hook to be rendered.');
	}
	return controls;
}

describe('useWorkbenchTerminalContextMenu', () => {
	beforeEach(() => {
		controls = null;
		document.body.replaceChildren();
	});

	it('leaves successful split focus transfer to the new terminal surface', () => {
		const splitPanel = vi.fn<(panelId: string, direction: PanelSplitDirection) => string | null>(
			() => 'panel-2',
		);
		renderToStaticMarkup(createElement(HookProjection, { splitPanel }));

		const contextMenu = getControls();
		contextMenu.split('right');

		expect(splitPanel).toHaveBeenCalledWith('panel-1', 'right', 'context-menu');
		expect(contextMenu.finalFocus()).toBe(false);
		expect(contextMenu.finalFocus()).toBe(false);
	});

	it('preserves the context menu focus behavior when a split is rejected', () => {
		renderToStaticMarkup(
			createElement(HookProjection, {
				splitPanel: () => null,
			}),
		);

		const contextMenu = getControls();
		contextMenu.split('right');

		expect(contextMenu.finalFocus()).toBe(false);
	});

	it('returns the keyboard to terminal input after the viewport opened a menu', () => {
		const viewport = document.createElement('div');
		const input = document.createElement('textarea');
		viewport.tabIndex = 0;
		viewport.append(input);
		document.body.append(viewport);
		viewport.focus();
		renderToStaticMarkup(
			createElement(HookProjection, {
				splitPanel: () => null,
			}),
		);

		const contextMenu = getControls();
		contextMenu.captureFinalFocus(viewport);

		expect(contextMenu.finalFocus()).toBe(input);
		expect(contextMenu.finalFocus()).toBe(false);
	});

	it('leaves focus with a surface the chosen command already opened', () => {
		const viewport = document.createElement('div');
		const input = document.createElement('textarea');
		const openedField = document.createElement('input');
		viewport.append(input);
		document.body.append(viewport, openedField);
		input.focus();
		renderToStaticMarkup(
			createElement(HookProjection, {
				splitPanel: () => null,
			}),
		);

		const contextMenu = getControls();
		contextMenu.captureFinalFocus(viewport);
		openedField.focus();

		expect(contextMenu.finalFocus()).toBe(false);
	});
});
