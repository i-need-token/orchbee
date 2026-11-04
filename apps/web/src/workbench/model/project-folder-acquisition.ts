import type {
	WorkbenchProjectFolderAcquisitionFailureCode,
	WorkbenchProjectFolderAcquisitionResult,
} from '../types/workbench-project-folder-acquisition';

export type WorkbenchProjectFolderAcquisitionState =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'failed'; readonly code: WorkbenchProjectFolderAcquisitionFailureCode }
	| { readonly kind: 'idle' }
	| { readonly kind: 'selecting' };

export type WorkbenchProjectFolderAcquisitionCommand =
	| { readonly kind: 'create-project'; readonly path: string }
	| { readonly kind: 'select-existing-folder' };

export type WorkbenchProjectFolderAcquisitionEvent =
	| { readonly kind: 'dismiss-failure' }
	| { readonly kind: 'dispose' }
	| { readonly kind: 'selection-settled'; readonly result: WorkbenchProjectFolderAcquisitionResult }
	| { readonly kind: 'selection-requested' };

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
			? { commands: [{ kind: 'select-existing-folder' }], state: { kind: 'selecting' } }
			: { commands: [], state };
	}
	if (state.kind !== 'selecting') return { commands: [], state };
	if (event.result.kind === 'cancelled') {
		return { commands: [], state: INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE };
	}
	if (event.result.kind === 'failed') {
		return { commands: [], state: { code: event.result.code, kind: 'failed' } };
	}
	return {
		commands: [{ kind: 'create-project', path: event.result.path }],
		state: INITIAL_WORKBENCH_PROJECT_FOLDER_ACQUISITION_STATE,
	};
}
