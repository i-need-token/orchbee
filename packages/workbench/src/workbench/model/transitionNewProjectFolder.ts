import type {
	WorkbenchNewProjectFolderParent,
	WorkbenchNewProjectFolderParentResult,
	WorkbenchNewProjectFolderResult,
	WorkbenchProjectFolderAcquisitionFailureCode,
} from '../types/workbench-project-folder-acquisition';

interface Draft {
	readonly parent: WorkbenchNewProjectFolderParent;
	readonly name: string;
}

export type NewProjectFolderState =
	| { readonly kind: 'idle' | 'selecting' | 'disposed' }
	| { readonly kind: 'failed'; readonly code: WorkbenchProjectFolderAcquisitionFailureCode }
	| (Draft & {
			readonly kind: 'draft';
			readonly failure: WorkbenchProjectFolderAcquisitionFailureCode | null;
	  })
	| (Draft & { readonly kind: 'creating' })
	| (Draft & { readonly kind: 'opening'; readonly path: string; readonly requestId: string })
	| (Draft & {
			readonly kind: 'recovery';
			readonly path: string | null;
			readonly requestId: string | null;
	  });

export type NewProjectFolderEvent =
	| { readonly kind: 'request' | 'cancel' | 'submit' | 'dispose' | 'retire' | 'recover' }
	| { readonly kind: 'parent-settled'; readonly result: WorkbenchNewProjectFolderParentResult }
	| { readonly kind: 'name-changed'; readonly name: string }
	| { readonly kind: 'creation-settled'; readonly result: WorkbenchNewProjectFolderResult }
	| { readonly kind: 'opening-settled'; readonly opened: boolean };

export type NewProjectFolderCommand =
	| { readonly kind: 'select-parent' | 'select-existing' }
	| { readonly kind: 'release-parent'; readonly parent: WorkbenchNewProjectFolderParent }
	| (Draft & { readonly kind: 'create' })
	| { readonly kind: 'open-project'; readonly path: string; readonly requestId: string }
	| { readonly kind: 'acknowledge'; readonly requestId: string };

interface NewProjectFolderTransition {
	readonly state: NewProjectFolderState;
	readonly commands: readonly NewProjectFolderCommand[];
}

export function transitionNewProjectFolder(
	state: NewProjectFolderState,
	event: NewProjectFolderEvent,
): NewProjectFolderTransition {
	if (state.kind === 'disposed') return { commands: [], state };
	if (event.kind === 'dispose')
		return {
			commands: 'parent' in state ? [{ kind: 'release-parent', parent: state.parent }] : [],
			state: { kind: 'disposed' },
		};
	if (event.kind === 'retire') {
		if (state.kind === 'opening') return { commands: [], state: { ...state, kind: 'recovery' } };
		if (state.kind === 'creating')
			return { commands: [], state: { ...state, kind: 'recovery', path: null, requestId: null } };
		if (state.kind === 'selecting' || state.kind === 'draft')
			return { commands: [], state: { code: 'control-unavailable', kind: 'failed' } };
	}
	if (event.kind === 'request' && state.kind === 'idle')
		return { commands: [{ kind: 'select-parent' }], state: { kind: 'selecting' } };
	if (event.kind === 'parent-settled' && state.kind === 'selecting') {
		const result = event.result;
		return result.kind === 'parent-selected'
			? { commands: [], state: { failure: null, kind: 'draft', name: '', parent: result.parent } }
			: {
					commands: [],
					state:
						result.kind === 'cancelled' ? { kind: 'idle' } : { code: result.code, kind: 'failed' },
				};
	}
	if (event.kind === 'name-changed' && state.kind === 'draft')
		return { commands: [], state: { ...state, failure: null, name: event.name } };
	if (event.kind === 'submit' && state.kind === 'draft' && state.name.trim().length > 0) {
		return {
			commands: [{ kind: 'create', name: state.name, parent: state.parent }],
			state: { kind: 'creating', name: state.name, parent: state.parent },
		};
	}
	if (event.kind === 'creation-settled' && state.kind === 'creating') {
		const result = event.result;
		if (result.kind === 'failed')
			return { commands: [], state: { ...state, failure: result.code, kind: 'draft' } };
		if (result.kind === 'created')
			return {
				commands: [{ kind: 'open-project', path: result.path, requestId: result.requestId }],
				state: { ...state, kind: 'opening', path: result.path, requestId: result.requestId },
			};
		return {
			commands: [],
			state: {
				...state,
				kind: 'recovery',
				path: result.kind === 'created-unavailable' ? result.path : null,
				requestId: result.requestId,
			},
		};
	}
	if (event.kind === 'opening-settled' && state.kind === 'opening') {
		return event.opened
			? { commands: [{ kind: 'acknowledge', requestId: state.requestId }], state: { kind: 'idle' } }
			: { commands: [], state: { ...state, kind: 'recovery' } };
	}
	if ((event.kind === 'cancel' || event.kind === 'recover') && state.kind === 'recovery') {
		return {
			commands: [
				...(state.requestId === null
					? []
					: [{ kind: 'acknowledge' as const, requestId: state.requestId }]),
				...(event.kind === 'recover' ? [{ kind: 'select-existing' as const }] : []),
			],
			state: { kind: 'idle' },
		};
	}
	if (event.kind === 'cancel' && (state.kind === 'draft' || state.kind === 'failed'))
		return {
			commands: state.kind === 'draft' ? [{ kind: 'release-parent', parent: state.parent }] : [],
			state: { kind: 'idle' },
		};
	return { commands: [], state };
}
