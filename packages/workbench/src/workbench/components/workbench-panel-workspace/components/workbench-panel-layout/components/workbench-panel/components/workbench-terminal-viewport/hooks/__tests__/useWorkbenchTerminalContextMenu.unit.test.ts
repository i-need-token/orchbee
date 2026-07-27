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
	});

	it('moves final focus to the newly split panel', () => {
		const focus = vi.fn<() => void>();
		const terminal = { focus } as unknown as HTMLElement;
		const querySelector = vi.fn<(selector: string) => HTMLElement | null>(() => terminal);
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>((callback) => {
			callback(0);
			return 1;
		});
		const ownerDocument = {
			defaultView: { requestAnimationFrame },
			querySelector,
		} as unknown as Document;
		const splitPanel = vi.fn<(panelId: string, direction: PanelSplitDirection) => string | null>(
			() => 'panel-2',
		);
		renderToStaticMarkup(createElement(HookProjection, { splitPanel }));

		const contextMenu = getControls();
		contextMenu.split('right', ownerDocument);

		expect(splitPanel).toHaveBeenCalledWith('panel-1', 'right', 'context-menu');
		expect(focus).toHaveBeenCalledOnce();
		expect(contextMenu.finalFocus()).toBe(terminal);
		expect(contextMenu.finalFocus()).toBe(true);
	});

	it('preserves the context menu focus behavior when a split is rejected', () => {
		const requestAnimationFrame = vi.fn<(callback: FrameRequestCallback) => number>(() => 1);
		const ownerDocument = {
			defaultView: { requestAnimationFrame },
			querySelector: vi.fn<(selector: string) => HTMLElement | null>(() => null),
		} as unknown as Document;
		renderToStaticMarkup(
			createElement(HookProjection, {
				splitPanel: () => null,
			}),
		);

		const contextMenu = getControls();
		contextMenu.split('right', ownerDocument);

		expect(requestAnimationFrame).not.toHaveBeenCalled();
		expect(contextMenu.finalFocus()).toBe(true);
	});
});
