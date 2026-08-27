import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { ProjectWorkspaceState } from '../../../../project-workspace';
import type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeDiscovery,
	WorkbenchProjectWorktreeFailureCode,
} from '../../../types/workbench-project-worktree';

interface UseProjectWorktreeMenuOptions {
	readonly capability: WorkbenchProjectWorktreeCapability | undefined;
	readonly projectWorkspaceState: ProjectWorkspaceState;
}

interface ProjectWorktreeMenuView {
	readonly adopt: (input: Parameters<WorkbenchProjectWorktreeCapability['adopt']>[0]) => void;
	readonly busy: boolean;
	readonly discoveryByProject: ReadonlyMap<string, WorkbenchProjectWorktreeDiscovery>;
	readonly discoveryFailureProjectIds: ReadonlySet<string>;
	readonly dismissFailure: () => void;
	readonly failure: WorkbenchProjectWorktreeFailureCode | null;
	readonly refresh: (projectId: string) => void;
}

interface ActiveRequest {
	readonly kind: 'adoption' | 'discovery';
	readonly projectId: string;
}

export function useProjectWorktreeMenu({
	capability,
	projectWorkspaceState,
}: UseProjectWorktreeMenuOptions): ProjectWorktreeMenuView {
	const activeRequest = React.useRef<ActiveRequest | null>(null);
	const capabilityRef = React.useRef(capability);
	capabilityRef.current = capability;
	const disposed = React.useRef(false);
	const projectWorkspaceStateRef = React.useRef(projectWorkspaceState);
	projectWorkspaceStateRef.current = projectWorkspaceState;
	const queuedProjectId = React.useRef<string | null>(null);
	const [busy, setBusy] = React.useState(false);
	const [discoveryByProject, setDiscoveryByProject] = React.useState<
		ReadonlyMap<string, WorkbenchProjectWorktreeDiscovery>
	>(new Map());
	const [discoveryFailureProjectIds, setDiscoveryFailureProjectIds] = React.useState<
		ReadonlySet<string>
	>(new Set());
	const [failure, setFailure] = React.useState<WorkbenchProjectWorktreeFailureCode | null>(null);

	React.useEffect(() => {
		disposed.current = false;
		return () => {
			disposed.current = true;
			queuedProjectId.current = null;
		};
	}, []);

	const startDiscovery = useMemoizedFn((projectId: string) => {
		const currentCapability = capabilityRef.current;
		const state = projectWorkspaceStateRef.current;
		const project = state.projects.find((candidate) => candidate.id === projectId);
		if (currentCapability === undefined || project === undefined || disposed.current) {
			setBusy(false);
			return;
		}
		const activeLocation = state.activeLocation;
		const activeWorkspace =
			activeLocation.kind === 'workspace' && activeLocation.projectId === projectId
				? project.workspaces.find((workspace) => workspace.id === activeLocation.workspaceId)
				: undefined;
		activeRequest.current = { kind: 'discovery', projectId };
		setBusy(true);
		void currentCapability
			.discover({
				preferredDirectoryId: activeWorkspace?.primaryDirectoryId ?? project.primaryDirectoryId,
				projectId,
			})
			.catch(() => ({ code: 'control-unavailable' as const, kind: 'failed' as const }))
			.then((result) => {
				if (disposed.current) return;
				if (result.kind === 'failed') {
					setDiscoveryByProject((current) => {
						const next = new Map(current);
						next.delete(projectId);
						return next;
					});
					setDiscoveryFailureProjectIds((current) => new Set(current).add(projectId));
					return;
				}
				setDiscoveryByProject((current) => new Map(current).set(projectId, result.discovery));
				setDiscoveryFailureProjectIds((current) => {
					const next = new Set(current);
					next.delete(projectId);
					return next;
				});
			})
			.finally(() => {
				if (disposed.current) return;
				activeRequest.current = null;
				const nextProjectId = queuedProjectId.current;
				queuedProjectId.current = null;
				if (nextProjectId === null) {
					setBusy(false);
				} else {
					startDiscovery(nextProjectId);
				}
			});
	});

	const refresh = useMemoizedFn((projectId: string) => {
		if (capabilityRef.current === undefined || disposed.current) return;
		const active = activeRequest.current;
		if (active !== null) {
			if (active.kind !== 'discovery' || active.projectId !== projectId) {
				queuedProjectId.current = projectId;
			}
			return;
		}
		startDiscovery(projectId);
	});

	const adopt = useMemoizedFn(
		(input: Parameters<WorkbenchProjectWorktreeCapability['adopt']>[0]) => {
			const currentCapability = capabilityRef.current;
			if (currentCapability === undefined || activeRequest.current !== null || disposed.current) {
				return;
			}
			activeRequest.current = { kind: 'adoption', projectId: input.projectId };
			setBusy(true);
			setFailure(null);
			void currentCapability
				.adopt(input)
				.catch(() => ({ code: 'control-unavailable' as const, kind: 'failed' as const }))
				.then((result) => {
					if (!disposed.current && result.kind === 'failed') setFailure(result.code);
				})
				.finally(() => {
					if (disposed.current) return;
					activeRequest.current = null;
					const nextProjectId = queuedProjectId.current;
					queuedProjectId.current = null;
					if (nextProjectId === null) {
						setBusy(false);
					} else {
						startDiscovery(nextProjectId);
					}
				});
		},
	);

	return {
		adopt,
		busy,
		discoveryByProject,
		discoveryFailureProjectIds,
		dismissFailure: useMemoizedFn(() => setFailure(null)),
		failure,
		refresh,
	};
}
