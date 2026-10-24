import type {
	WorkbenchProjectFolderAcquisitionFailureCode,
	WorkbenchProjectFolderAcquisitionResult,
} from '../types/workbench-project-folder-acquisition';

type WorkbenchProjectFolderAcquisitionTarget =
	| { readonly kind: 'create-project' }
	| { readonly kind: 'add-project-folder'; readonly resourceScopeId: string };

export type WorkbenchProjectFolderAcquisitionState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'failed'; readonly code: WorkbenchProjectFolderAcquisitionFailureCode }
	| { readonly kind: 'idle' }
	| { readonly kind: 'selecting'; readonly target: WorkbenchProjectFolderAcquisitionTarget };

export type WorkbenchProjectFolderAcquisitionCommand =
	| { readonly kind: 'add-project-folder'; readonly path: string; readonly resourceScopeId: string }
	| { readonly kind: 'create-project'; readonly path: string }
	| { readonly kind: 'select-existing-folder' };

export type WorkbenchProjectFolderAcquisitionEvent =
	| { readonly kind: 'dismiss-failure' }
	| { readonly kind: 'dispose' }
	| {
			readonly addTargetStatus?: 'available' | 'duplicate' | 'missing' | undefined;
			readonly kind: 'selection-settled';
			readonly result: WorkbenchProjectFolderAcquisitionResult;
	  }
	| {
			readonly kind: 'selection-requested';
			readonly target: WorkbenchProjectFolderAcquisitionTarget;
	  };

interface WorkbenchProjectFolderAcquisitionTransition {
	readonly commands: readonly WorkbenchProjectFolderAcquisitionCommand[];
	readonly state: WorkbenchProjectFolderAcquisitionState;
}

export const INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE = {
	kind: 'idle',
} as const satisfies WorkbenchProjectFolderAcquisitionState;

export function transitionWorkbenchProjectFolderAcquisition(
	state: WorkbenchProjectFolderAcquisitionState,
	event: WorkbenchProjectFolderAcquisitionEvent,
): WorkbenchProjectFolderAcquisitionTransition {
	if (event.kind === 'dispose') return { commands: [], state: { kind: 'disposed' } };
	if (state.kind === 'disposed') return { commands: [], state };
	if (event.kind === 'dismiss-failure') {
		return state.kind === 'failed'
			? { commands: [], state: INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE }
			: { commands: [], state };
	}
	if (event.kind === 'selection-requested') {
		return state.kind === 'idle'
			? {
					commands: [{ kind: 'select-existing-folder' }],
					state: { kind: 'selecting', target: event.target },
				}
			: { commands: [], state };
	}
	if (state.kind !== 'selecting') return { commands: [], state };
	if (event.result.kind === 'cancelled') {
		return { commands: [], state: INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE };
	}
	if (event.result.kind === 'failed') {
		return { commands: [], state: { code: event.result.code, kind: 'failed' } };
	}
	if (state.target.kind === 'create-project') {
		return {
			commands: [{ kind: 'create-project', path: event.result.path }],
			state: INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
		};
	}
	if (event.addTargetStatus !== 'available') {
		return {
			commands: [],
			state: {
				code: event.addTargetStatus === 'duplicate' ? 'already-added' : 'project-unavailable',
				kind: 'failed',
			},
		};
	}
	return {
		commands: [
			{
				kind: 'add-project-folder',
				path: event.result.path,
				resourceScopeId: state.target.resourceScopeId,
			},
		],
		state: INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	};
}
