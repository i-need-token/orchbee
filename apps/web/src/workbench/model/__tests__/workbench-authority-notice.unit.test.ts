import { describe, expect, it } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureContextId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import {
	readWorkbenchCloseSubject,
	workbenchCloseProgressNotice,
} from '../workbench-authority-notice';

function tabs(...sequences: number[]) {
	return sequences.map((sequence) => snapshotTab(sequence));
}

describe('workbenchCloseProgressNotice', () => {
	it('counts a member of a closing set as closed only once the authority no longer holds it', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, tabs(1, 2, 3)));
		const subject = readWorkbenchCloseSubject(
			{
				tabIds: [fixtureTabId(1), fixtureTabId(2)],
				terminalContextId: fixtureContextId(1),
				type: 'tabs-close',
			},
			runtime,
		);

		// Both members leave the strip once the set is accepted, while the authority still holds them.
		publishSnapshot(runtime, oneWorkspaceSnapshot(2, tabs(1, 2, 3)), [
			fixtureTabId(1),
			fixtureTabId(2),
		]);
		expect(workbenchCloseProgressNotice(subject, runtime)?.values).toEqual({
			completed: 0,
			remaining: 2,
		});

		publishSnapshot(runtime, oneWorkspaceSnapshot(3, tabs(2, 3)), [fixtureTabId(2)]);
		expect(workbenchCloseProgressNotice(subject, runtime)?.values).toEqual({
			completed: 1,
			remaining: 1,
		});
	});
});
