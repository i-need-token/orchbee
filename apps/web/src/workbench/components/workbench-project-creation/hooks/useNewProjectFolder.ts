import type { ActionExecution } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import {
	transitionNewProjectFolder,
	type NewProjectFolderCommand,
	type NewProjectFolderEvent,
	type NewProjectFolderState,
} from '../../../model/transitionNewProjectFolder';
import type {
	WorkbenchNewProjectFolderCapability,
	WorkbenchProjectCreationReceipt,
} from '../../../types/workbench-project-folder-acquisition';

interface UseNewProjectFolderOptions {
	readonly capability: WorkbenchNewProjectFolderCapability | undefined;
	readonly createProject: (path: string) => ActionExecution<WorkbenchProjectCreationReceipt>;
	readonly activateCreatedProject: (
		receipt: WorkbenchProjectCreationReceipt,
	) => ActionExecution<void>;
	readonly isCreatedProjectActive: (receipt: WorkbenchProjectCreationReceipt) => boolean;
	readonly selectExisting: () => void;
}

export interface NewProjectFolderView {
	readonly state: NewProjectFolderState;
	readonly busy: boolean;
	readonly open: () => void;
	readonly close: () => void;
	readonly recover: () => void;
	readonly setName: (name: string) => void;
	readonly submit: () => void;
}

export function useNewProjectFolder({
	capability,
	createProject,
	activateCreatedProject,
	isCreatedProjectActive,
	selectExisting,
}: UseNewProjectFolderOptions): NewProjectFolderView {
	const [state, setState] = React.useState<NewProjectFolderState>({ kind: 'idle' });
	const model = React.useRef<NewProjectFolderState>(state);
	const generation = React.useRef(0);
	const mounted = React.useRef(false);
	const actions = React.useRef(new Set<ActionExecution<unknown>>());
	const apply = useMemoizedFn((event: NewProjectFolderEvent) => {
		const transition = transitionNewProjectFolder(model.current, event);
		model.current = transition.state;
		if (mounted.current) setState(transition.state);
		for (const command of transition.commands) execute(command);
	});
	const execute = useMemoizedFn((command: NewProjectFolderCommand) => {
		const owner = capability;
		const version = generation.current;
		const live = () =>
			mounted.current &&
			generation.current === version &&
			owner !== undefined &&
			!owner.signal.aborted;
		if (command.kind === 'select-existing') {
			selectExisting();
			return;
		}
		if (owner === undefined) return;
		switch (command.kind) {
			case 'acknowledge':
				owner.acknowledge(command.requestId);
				break;
			case 'release-parent':
				owner.releaseParent(command.parent);
				break;
			case 'select-parent':
				void owner
					.selectParent()
					.then((result) => {
						if (live()) apply({ kind: 'parent-settled', result });
						else if (result.kind === 'parent-selected') owner.releaseParent(result.parent);
					})
					.catch(() => {
						if (live())
							apply({
								kind: 'parent-settled',
								result: { code: 'control-unavailable', kind: 'failed' },
							});
					});
				break;
			case 'create':
				void owner
					.create(command.parent, command.name)
					.then((result) => {
						if (live()) apply({ kind: 'creation-settled', result });
						else owner.releaseParent(command.parent);
					})
					.catch(() => {
						if (live()) apply({ kind: 'retire' });
						owner.releaseParent(command.parent);
					});
				break;
			case 'open-project': {
				const current = () =>
					live() &&
					model.current.kind === 'opening' &&
					model.current.requestId === command.requestId;
				const run = async (): Promise<void> => {
					if (!current()) return;
					const creation = createProject(command.path);
					actions.current.add(creation);
					if (!current()) creation.cancel();
					const result = await creation.completion;
					actions.current.delete(creation);
					if (!current()) return;
					if (
						result.kind !== 'completed' ||
						result.value === undefined ||
						result.value.path !== command.path
					) {
						apply({ kind: 'opening-settled', opened: false });
						return;
					}
					const receipt = result.value;
					const published = await owner.confirmProject(receipt);
					if (!current()) return;
					if (!published) {
						apply({ kind: 'opening-settled', opened: false });
						return;
					}
					const activation = activateCreatedProject(receipt);
					actions.current.add(activation);
					if (!current()) activation.cancel();
					const activated = await activation.completion;
					actions.current.delete(activation);
					if (!current()) return;
					if (activated.kind !== 'completed' && activated.kind !== 'unchanged') {
						apply({ kind: 'opening-settled', opened: false });
						return;
					}
					const stillPublished = await owner.confirmProject(receipt);
					if (!current()) return;
					apply({
						kind: 'opening-settled',
						opened: stillPublished && isCreatedProjectActive(receipt),
					});
				};
				void run().catch(() => {
					if (current()) apply({ kind: 'opening-settled', opened: false });
				});
				break;
			}
		}
	});
	React.useEffect(() => {
		const pendingActions = actions.current;
		mounted.current = true;
		model.current = { kind: 'idle' };
		setState(model.current);
		const retire = () => {
			for (const action of pendingActions) action.cancel();
			pendingActions.clear();
			apply({ kind: 'retire' });
		};
		capability?.signal.addEventListener('abort', retire);
		return () => {
			mounted.current = false;
			generation.current += 1;
			capability?.signal.removeEventListener('abort', retire);
			for (const action of pendingActions) action.cancel();
			pendingActions.clear();
			const current = model.current;
			if ('parent' in current) capability?.releaseParent(current.parent);
			model.current = { kind: 'disposed' };
		};
	}, [apply, capability]);
	return {
		busy: state.kind !== 'idle' && state.kind !== 'disposed',
		close: useMemoizedFn(() => apply({ kind: 'cancel' })),
		open: useMemoizedFn(() => {
			if (capability !== undefined && !capability.signal.aborted) apply({ kind: 'request' });
		}),
		recover: useMemoizedFn(() => apply({ kind: 'recover' })),
		setName: useMemoizedFn((name: string) => apply({ kind: 'name-changed', name })),
		state,
		submit: useMemoizedFn(() => apply({ kind: 'submit' })),
	};
}
