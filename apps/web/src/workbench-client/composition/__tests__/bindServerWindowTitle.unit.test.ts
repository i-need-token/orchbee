import { parseWorkbenchRequest } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	createSnapshotRuntime,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import type { ServerConnection } from '../../server-connection';
import { bindServerWindowTitle } from '../bindServerWindowTitle';

type Handlers = Parameters<ServerConnection['subscribe']>[2];

/** Records the Tab name each window-title listen carries, as the Server receives it. */
class ListenRecorder {
	readonly tabs: string[] = [];

	subscribe(_mode: string, request: Uint8Array, _handlers: Handlers) {
		const parsed = parseWorkbenchRequest(
			JSON.parse(new TextDecoder().decode(request.subarray(12))),
		);
		if (parsed.kind === 'workbench.window-title' && parsed.action.kind === 'listen') {
			this.tabs.push(parsed.action.context.tab);
		}
		return { close: () => {}, resnapshot: () => {} };
	}
}

/** An automatically named Tab whose terminal the Server launched with `program`. */
function automaticTab(program: string) {
	return snapshotTab(1, snapshotPanel(1, snapshotPresentation(1, { launchProgram: program })), {
		automaticLabel: true,
		label: '1',
	});
}

describe('bindServerWindowTitle', () => {
	it('names the window after the Tab as its strip shows it, and renames it as that name moves', () => {
		const first = oneWorkspaceSnapshot(1, [automaticTab('pwsh.exe')]);
		const runtime = createSnapshotRuntime(first);
		const connection = new ListenRecorder();
		const binding = bindServerWindowTitle({
			connection,
			runtime,
			session: { snapshot: first, subscribe: () => () => {} },
			terminalTitle: () => null,
			titleWindow: {
				document: { hasFocus: () => true, title: '' },
				addEventListener: () => {},
				removeEventListener: () => {},
			},
		});
		try {
			expect(connection.tabs.at(-1)).toBe('PowerShell');
			publishSnapshot(runtime, oneWorkspaceSnapshot(2, [automaticTab('node.exe')]));
			expect(connection.tabs.at(-1)).toBe('node');
		} finally {
			binding.dispose();
		}
	});
});
