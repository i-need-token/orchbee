import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { resolveActiveTerminalPresentationId } from '../resolveActiveTerminalPresentationId';

describe('resolveActiveTerminalPresentationId', () => {
	it('returns only the focused terminal in the active Tab and becomes unavailable without one', () => {
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('terminal-initial'),
				() => terminalPresentationId('terminal-next'),
			),
		});
		const terminalContext = runtime.getActiveTerminalContext();

		expect(resolveActiveTerminalPresentationId(runtime)).toBe('terminal-initial');

		const activeTabId = terminalContext.tabModel.getSnapshot().activeTabId;
		if (activeTabId === null) throw new Error('Expected an active Tab in the test runtime.');
		terminalContext.tabModel.closeTab(activeTabId);

		expect(resolveActiveTerminalPresentationId(runtime)).toBeNull();
	});
});
