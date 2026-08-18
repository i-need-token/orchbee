import { describe, expect, it } from 'vitest';

import {
	INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	transitionWorkbenchProjectFolderAcquisition,
} from '../project-folder-acquisition';

describe('transitionWorkbenchProjectFolderAcquisition', () => {
	it('commits only the matching selected create request', () => {
		const selecting = transitionWorkbenchProjectFolderAcquisition(
			INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
			{ kind: 'selection-requested', target: { kind: 'create-project' } },
		);
		expect(selecting.commands).toEqual([{ kind: 'select-existing-folder' }]);

		expect(
			transitionWorkbenchProjectFolderAcquisition(selecting.state, {
				kind: 'selection-settled',
				result: { kind: 'selected', path: '/workspace/project' },
			}),
		).toEqual({
			commands: [{ kind: 'create-project', path: '/workspace/project' }],
			state: { kind: 'idle' },
		});
	});

	it('rejects stale and duplicate add targets without mutation', () => {
		const selecting = transitionWorkbenchProjectFolderAcquisition(
			INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
			{
				kind: 'selection-requested',
				target: { kind: 'add-project-folder', projectId: 'project-1' },
			},
		).state;
		for (const [addTargetStatus, code] of [
			['missing', 'project-unavailable'],
			['duplicate', 'already-added'],
		] as const) {
			expect(
				transitionWorkbenchProjectFolderAcquisition(selecting, {
					addTargetStatus,
					kind: 'selection-settled',
					result: { kind: 'selected', path: '/workspace/project' },
				}),
			).toEqual({ commands: [], state: { code, kind: 'failed' } });
		}
	});

	it('makes cancellation, failure, disposal, and late settlement mutation-free', () => {
		const selecting = transitionWorkbenchProjectFolderAcquisition(
			INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
			{ kind: 'selection-requested', target: { kind: 'create-project' } },
		).state;
		expect(
			transitionWorkbenchProjectFolderAcquisition(selecting, {
				kind: 'selection-settled',
				result: { kind: 'cancelled' },
			}),
		).toEqual({ commands: [], state: { kind: 'idle' } });
		expect(
			transitionWorkbenchProjectFolderAcquisition(selecting, {
				kind: 'selection-settled',
				result: { code: 'permission-denied', kind: 'failed' },
			}),
		).toEqual({ commands: [], state: { code: 'permission-denied', kind: 'failed' } });

		const disposed = transitionWorkbenchProjectFolderAcquisition(selecting, { kind: 'dispose' });
		expect(
			transitionWorkbenchProjectFolderAcquisition(disposed.state, {
				kind: 'selection-settled',
				result: { kind: 'selected', path: '/workspace/late' },
			}),
		).toEqual({ commands: [], state: { kind: 'disposed' } });
	});
});
