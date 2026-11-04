// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import { useWorkbenchPanelResizeDialog } from '../useWorkbenchPanelResizeDialog';

afterEach(() => {
	cleanup();
	document.body.replaceChildren();
});

describe('useWorkbenchPanelResizeDialog', () => {
	it('reports an unavailable resize and restores focus to the captured Panel', async () => {
		const resizePanel = vi.fn<WorkbenchPanelWorkspaceView['resizePanel']>(() =>
			Promise.resolve(false),
		);
		const host = {
			messages: { resizeUnavailable: 'Cannot resize farther' },
			pendingResize: { panelId: 'panel-2', tabId: 'tab-1' },
			resizePanel,
		} satisfies Parameters<typeof useWorkbenchPanelResizeDialog>[0];
		const viewport = document.createElement('div');
		viewport.dataset['panelId'] = 'panel-2';
		viewport.dataset['slot'] = 'workbench-terminal-viewport';
		const input = document.createElement('textarea');
		viewport.append(input);
		document.body.append(viewport);
		const rendered = renderHook(() => useWorkbenchPanelResizeDialog(host));

		await act(async () => rendered.result.current.resize('wider', 2));

		expect(resizePanel).toHaveBeenCalledExactlyOnceWith('panel-2', 'wider', 2);
		expect(rendered.result.current.status).toBe('Cannot resize farther');
		expect(rendered.result.current.finalFocus()).toBe(input);
	});

	it('keeps successful resize status quiet when no target has gone stale', async () => {
		const resizePanel = vi.fn<WorkbenchPanelWorkspaceView['resizePanel']>(() =>
			Promise.resolve(true),
		);
		const host = {
			messages: { resizeUnavailable: 'Cannot resize farther' },
			pendingResize: { panelId: 'panel-1', tabId: 'tab-1' },
			resizePanel,
		} satisfies Parameters<typeof useWorkbenchPanelResizeDialog>[0];
		const rendered = renderHook(() => useWorkbenchPanelResizeDialog(host));

		await act(async () => rendered.result.current.resize('reset', 1));

		expect(rendered.result.current.status).toBeNull();
	});
});
