import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../../project-workspace';
import type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeDiscovery,
	WorkbenchProjectWorktreeFailureCode,
} from '../../../types/workbench-project-worktree';

interface ProjectWorktreeDialogState {
	readonly baseRef: string;
	readonly branchName: string;
	readonly createWorkspace: boolean;
	readonly discovery: WorkbenchProjectWorktreeDiscovery | null;
	readonly failure: WorkbenchProjectWorktreeFailureCode | null;
	readonly phase: 'discovering' | 'ready' | 'submitting';
	readonly projectId: string;
	readonly repositoryId: string;
}

interface UseProjectWorktreeCreationOptions {
	readonly capability: WorkbenchProjectWorktreeCapability | undefined;
	readonly projectWorkspaceState: ProjectWorkspaceState;
	readonly requestWorkspaceActivation: (projectId: string, workspaceId: string) => void;
}

interface ProjectWorktreeCreationView {
	readonly close: () => void;
	readonly dialog: ProjectWorktreeDialogState | null;
	readonly open: (projectId: string) => void;
	readonly setBaseRef: (baseRef: string) => void;
	readonly setBranchName: (branchName: string) => void;
	readonly setCreateWorkspace: (createWorkspace: boolean) => void;
	readonly setRepository: (repositoryId: string) => void;
	readonly submit: () => void;
}

export function useProjectWorktreeCreation({
	capability,
	projectWorkspaceState,
	requestWorkspaceActivation,
}: UseProjectWorktreeCreationOptions): ProjectWorktreeCreationView {
	const requestGeneration = React.useRef(0);
	const [dialog, setDialog] = React.useState<ProjectWorktreeDialogState | null>(null);

	React.useEffect(
		() => () => {
			requestGeneration.current += 1;
		},
		[],
	);

	const close = useMemoizedFn(() => {
		requestGeneration.current += 1;
		setDialog(null);
	});
	const open = useMemoizedFn((projectId: string) => {
		if (capability === undefined) return;
		const project = projectWorkspaceState.projects.find((candidate) => candidate.id === projectId);
		if (project === undefined) return;
		const activeLocation = projectWorkspaceState.activeLocation;
		const activeWorkspace =
			activeLocation.kind === 'workspace' && activeLocation.projectId === projectId
				? project.workspaces.find((workspace) => workspace.id === activeLocation.workspaceId)
				: undefined;
		const generation = ++requestGeneration.current;
		setDialog({
			baseRef: '',
			branchName: '',
			createWorkspace: true,
			discovery: null,
			failure: null,
			phase: 'discovering',
			projectId,
			repositoryId: '',
		});
		void capability
			.discover({
				preferredDirectoryId: activeWorkspace?.primaryDirectoryId ?? project.primaryDirectoryId,
				projectId,
			})
			.catch(() => ({ code: 'control-unavailable' as const, kind: 'failed' as const }))
			.then((result) => {
				if (requestGeneration.current !== generation) return;
				if (result.kind === 'failed') {
					setDialog((current) =>
						current?.projectId === projectId
							? { ...current, failure: result.code, phase: 'ready' }
							: current,
					);
					return;
				}
				const repository = result.discovery.repositories[0];
				setDialog((current) =>
					current?.projectId === projectId
						? {
								...current,
								baseRef: repository?.defaultBaseRef ?? '',
								branchName: repository?.suggestedBranchName ?? '',
								discovery: result.discovery,
								phase: 'ready',
								repositoryId: repository?.repositoryId ?? '',
							}
						: current,
				);
			});
	});
	const submit = useMemoizedFn(() => {
		if (capability === undefined || dialog?.phase !== 'ready') return;
		const repository = dialog.discovery?.repositories.find(
			(candidate) => candidate.repositoryId === dialog.repositoryId,
		);
		if (repository === undefined) return;
		const generation = requestGeneration.current;
		const submitted = dialog;
		setDialog({ ...submitted, failure: null, phase: 'submitting' });
		void capability
			.create({
				baseRef: submitted.baseRef.trim(),
				branchName: submitted.branchName.trim(),
				createWorkspace: submitted.createWorkspace,
				projectId: submitted.projectId,
				repositoryId: repository.repositoryId,
				sourceDirectoryId: repository.sourceDirectoryId,
			})
			.catch(() => ({ code: 'control-unavailable' as const, kind: 'failed' as const }))
			.then((result) => {
				if (result.kind === 'created') {
					if (requestGeneration.current !== generation) return;
					close();
					if (result.workspaceId !== null) {
						requestWorkspaceActivation(submitted.projectId, result.workspaceId);
					}
					return;
				}
				if (requestGeneration.current !== generation) return;
				setDialog((current) =>
					current === null ? null : { ...current, failure: result.code, phase: 'ready' },
				);
			});
	});

	return {
		close,
		dialog,
		open,
		setBaseRef: useMemoizedFn((baseRef: string) =>
			setDialog((current) => (current === null ? null : { ...current, baseRef, failure: null })),
		),
		setBranchName: useMemoizedFn((branchName: string) =>
			setDialog((current) => (current === null ? null : { ...current, branchName, failure: null })),
		),
		setCreateWorkspace: useMemoizedFn((shouldCreateWorkspace: boolean) =>
			setDialog((current) =>
				current === null ? null : { ...current, createWorkspace: shouldCreateWorkspace },
			),
		),
		setRepository: useMemoizedFn((repositoryId: string) =>
			setDialog((current) => {
				const repository = current?.discovery?.repositories.find(
					(candidate) => candidate.repositoryId === repositoryId,
				);
				return current === null || repository === undefined
					? current
					: {
							...current,
							baseRef: repository.defaultBaseRef,
							branchName: repository.suggestedBranchName,
							failure: null,
							repositoryId,
						};
			}),
		),
		submit,
	};
}
