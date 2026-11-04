import { describe, expect, it } from 'vitest';
import { transitionNewProjectFolder } from '../transitionNewProjectFolder';

import {
	INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	transitionWorkbenchProjectFolderAcquisition,
} from '../project-folder-acquisition';

describe('transitionWorkbenchProjectFolderAcquisition', () => {
	it('requires explicit Create, preserves a parent on conflict, and never replays a committed mkdir', () => {
		const parent = { path: '/workspace', selectionId: 'parent:1' };
		const selecting = transitionNewProjectFolder({ kind: 'idle' }, { kind: 'request' });
		const draft = transitionNewProjectFolder(selecting.state, {
			kind: 'parent-settled',
			result: { kind: 'parent-selected', parent },
		});
		expect(draft.commands).toEqual([]);
		const named = transitionNewProjectFolder(draft.state, {
			kind: 'name-changed',
			name: 'orchbee',
		}).state;
		expect(transitionNewProjectFolder(named, { kind: 'cancel' }).commands).toEqual([
			{ kind: 'release-parent', parent },
		]);
		const creating = transitionNewProjectFolder(named, { kind: 'submit' });
		expect(transitionNewProjectFolder(creating.state, { kind: 'submit' }).commands).toEqual([]);
		const conflict = transitionNewProjectFolder(creating.state, {
			kind: 'creation-settled',
			result: { code: 'already-exists', kind: 'failed' },
		});
		expect(conflict.state).toMatchObject({ failure: 'already-exists', kind: 'draft', parent });
		const retry = transitionNewProjectFolder(conflict.state, { kind: 'submit' });
		const opening = transitionNewProjectFolder(retry.state, {
			kind: 'creation-settled',
			result: { kind: 'created', path: '/workspace/orchbee', requestId: 'create:2' },
		});
		expect(opening.commands).toEqual([
			{ kind: 'open-project', path: '/workspace/orchbee', requestId: 'create:2' },
		]);
		const recovery = transitionNewProjectFolder(opening.state, {
			kind: 'opening-settled',
			opened: false,
		});
		expect(transitionNewProjectFolder(recovery.state, { kind: 'submit' }).commands).toEqual([]);
		expect(transitionNewProjectFolder(recovery.state, { kind: 'recover' }).commands).toEqual([
			{ kind: 'acknowledge', requestId: 'create:2' },
			{ kind: 'select-existing' },
		]);
	});

	it('preserves known and unknown effects on retirement without issuing late opening', () => {
		const state = {
			kind: 'creating',
			name: 'new',
			parent: { path: '/workspace', selectionId: 'parent:1' },
		} as const;
		const retired = transitionNewProjectFolder(state, { kind: 'retire' });
		expect(retired.state).toMatchObject({ kind: 'recovery', path: null });
		expect(
			transitionNewProjectFolder(retired.state, {
				kind: 'creation-settled',
				result: { kind: 'created', path: '/workspace/new', requestId: 'create:1' },
			}).commands,
		).toEqual([]);
		const disposed = transitionNewProjectFolder(state, { kind: 'dispose' });
		expect(disposed.commands).toEqual([{ kind: 'release-parent', parent: state.parent }]);
		expect(transitionNewProjectFolder(disposed.state, { kind: 'submit' }).commands).toEqual([]);
	});
	it('commits only the matching selected create request', () => {
		const selecting = transitionWorkbenchProjectFolderAcquisition(
			INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
			{ kind: 'selection-requested' },
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

	it('makes cancellation, failure, disposal, and late settlement mutation-free', () => {
		const selecting = transitionWorkbenchProjectFolderAcquisition(
			INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
			{ kind: 'selection-requested' },
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
