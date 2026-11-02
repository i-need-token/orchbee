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

	it('restores the element focused before a product context menu opened', () => {
		class FakeHTMLElement {
			ownerDocument: unknown;
		}
		const activeElement = new FakeHTMLElement() as unknown as HTMLElement;
		const ownerDocument = {
			activeElement,
			defaultView: { HTMLElement: FakeHTMLElement },
		} as unknown as Document;
		(activeElement as unknown as FakeHTMLElement).ownerDocument = ownerDocument;
		renderToStaticMarkup(
			createElement(HookProjection, {
				splitPanel: () => null,
			}),
		);

		const contextMenu = getControls();
		contextMenu.captureFinalFocus(ownerDocument);

		expect(contextMenu.finalFocus()).toBe(activeElement);
		expect(contextMenu.finalFocus()).toBe(false);
	});

	it('leaves focus with a surface the chosen command already opened', () => {
		class FakeHTMLElement {
			ownerDocument: unknown;
		}
		const menuInvoker = new FakeHTMLElement() as unknown as HTMLElement;
		const ownerDocument = {
			activeElement: menuInvoker,
			body: new FakeHTMLElement(),
			defaultView: { HTMLElement: FakeHTMLElement },
		};
		(menuInvoker as unknown as FakeHTMLElement).ownerDocument = ownerDocument;
		renderToStaticMarkup(
			createElement(HookProjection, {
				splitPanel: () => null,
			}),
		);

		const contextMenu = getControls();
		contextMenu.captureFinalFocus(ownerDocument as unknown as Document);
		ownerDocument.activeElement = { closest: () => null } as unknown as HTMLElement;

		expect(contextMenu.finalFocus()).toBe(false);
	});
});
