// @vitest-environment jsdom

import { ActionRuntime, type ActionExecution } from '@orchbee/actions';
import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { FileOverlayOpenFile } from '../../../file-overlay';
import { FileSessionRegistry, MemoryFileSessionPort } from '../../../file-session';
import { useWorkbenchTerminalFileLinks } from '../useWorkbenchTerminalFileLinks';

const WORKSPACE = 'workspace-1';

/** An open the overlay finished at once, as it opens a file it can show. */
const OPENED: ActionExecution<void> = {
	cancel: () => undefined,
	completion: Promise.resolve({ kind: 'completed' }),
	executionId: 'opened',
	getSnapshot: () => ({ progress: null, result: { kind: 'completed' }, state: 'completed' }),
	subscribe: () => ({ dispose: () => undefined }),
};

afterEach(cleanup);

describe('useWorkbenchTerminalFileLinks', () => {
	// The terminal writes every link's path with `/`, a Windows machine's paths included.
	it("opens a link to a Windows machine's file by the path that machine writes", async () => {
		const port = new MemoryFileSessionPort({
			rules: { caseSensitive: false, os: 'windows', separator: '\\' },
		});
		port.writeFile('C:\\w\\src\\a.ts', 'a');
		port.setRoot(WORKSPACE, 'C:\\w');
		const opened: string[] = [];
		const openFile: FileOverlayOpenFile = (input) => {
			opened.push(input.resource.path);
			return OPENED;
		};
		const options = {
			actionScope: new ActionRuntime().createScope('workbench'),
			offered: true,
			openFile,
			sessions: new FileSessionRegistry(port),
			workspaceId: WORKSPACE,
		};
		const { result } = renderHook(() => useWorkbenchTerminalFileLinks(options));

		const outcome = await result.current?.({ path: 'C:/w/src/a.ts' }, new AbortController().signal);

		expect(outcome).toEqual({ kind: 'completed' });
		expect(opened).toEqual(['C:\\w\\src\\a.ts']);
	});
});
