import { describe, expect, it } from 'vitest';

import {
	createSnapshotRuntime,
	fixturePresentationId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { resolveActiveTerminalPresentationId } from '../resolveActiveTerminalPresentationId';

describe('resolveActiveTerminalPresentationId', () => {
	it('returns only the focused terminal in the active Tab and becomes unavailable without one', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));

		expect(resolveActiveTerminalPresentationId(runtime)).toBe(fixturePresentationId(1));

		publishSnapshot(runtime, oneWorkspaceSnapshot(2, []));

		expect(resolveActiveTerminalPresentationId(runtime)).toBeNull();
	});
});
