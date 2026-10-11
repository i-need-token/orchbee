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
	readonly beginWorkspaceActivation: () => (workspaceId: string) => void;
}

interface ProjectWorktreeMenuView {
	readonly adopt: (input: Parameters<WorkbenchProjectWorktreeCapability['adopt']>[0]) => void;
	readonly busy: boolean;
	readonly discoveryByWorkspace: ReadonlyMap<string, WorkbenchProjectWorktreeDiscovery>;
	readonly discoveryFailureWorkspaceIds: ReadonlySet<string>;
	readonly dismissFailure: () => void;
	readonly failure: WorkbenchProjectWorktreeFailureCode | null;
	readonly refresh: (sourceWorkspaceId: string) => void;
}

interface ActiveRequest {
	readonly kind: 'adoption' | 'discovery';
	readonly sourceWorkspaceId: string;
}

export function useProjectWorktreeMenu({
	capability,
	projectWorkspaceState,
	beginWorkspaceActivation,
}: UseProjectWorktreeMenuOptions): ProjectWorktreeMenuView {
	const activeRequest = React.useRef<ActiveRequest | null>(null);
	const disposed = React.useRef(false);
	const queuedWorkspaceId = React.useRef<string | null>(null);
	const [busy, setBusy] = React.useState(false);
	const [discoveryByWorkspace, setDiscoveryByWorkspace] = React.useState<
		ReadonlyMap<string, WorkbenchProjectWorktreeDiscovery>
	>(new Map());
	const [discoveryFailureWorkspaceIds, setDiscoveryFailureWorkspaceIds] = React.useState<
		ReadonlySet<string>
	>(new Set());
	const [failure, setFailure] = React.useState<WorkbenchProjectWorktreeFailureCode | null>(null);

	React.useEffect(() => {
		disposed.current = false;
		return () => {
			disposed.current = true;
			queuedWorkspaceId.current = null;
		};
	}, []);

	const startDiscovery = useMemoizedFn((sourceWorkspaceId: string) => {
		const currentCapability = capability;
		const state = projectWorkspaceState;
		const sourceWorkspace = state.workspaces.find(
			(candidate) => candidate.id === sourceWorkspaceId,
		);
		if (currentCapability === undefined || sourceWorkspace === undefined || disposed.current) {
			setBusy(false);
			return;
		}
		activeRequest.current = { kind: 'discovery', sourceWorkspaceId };
		setBusy(true);
		void currentCapability
			.discover({
				preferredDirectoryId: sourceWorkspace.primaryDirectoryId,
				sourceWorkspaceId,
			})
			.catch(() => ({ code: 'control-unavailable' as const, kind: 'failed' as const }))
			.then((result) => {
				if (disposed.current) return;
				if (result.kind === 'failed') {
					setDiscoveryByWorkspace((current) => {
						const next = new Map(current);
						next.delete(sourceWorkspaceId);
						return next;
					});
					setDiscoveryFailureWorkspaceIds((current) => new Set(current).add(sourceWorkspaceId));
					return;
				}
				setDiscoveryByWorkspace((current) =>
					new Map(current).set(sourceWorkspaceId, result.discovery),
				);
				setDiscoveryFailureWorkspaceIds((current) => {
					const next = new Set(current);
					next.delete(sourceWorkspaceId);
					return next;
				});
			})
			.finally(() => {
				if (disposed.current) return;
				activeRequest.current = null;
				const nextWorkspaceId = queuedWorkspaceId.current;
				queuedWorkspaceId.current = null;
				if (nextWorkspaceId === null) {
					setBusy(false);
				} else {
					startDiscovery(nextWorkspaceId);
				}
			});
	});

	const refresh = useMemoizedFn((sourceWorkspaceId: string) => {
		if (capability === undefined || disposed.current) return;
		const active = activeRequest.current;
		if (active !== null) {
			if (active.kind !== 'discovery' || active.sourceWorkspaceId !== sourceWorkspaceId) {
				queuedWorkspaceId.current = sourceWorkspaceId;
			}
			return;
		}
		startDiscovery(sourceWorkspaceId);
	});

	const adopt = useMemoizedFn(
		(input: Parameters<WorkbenchProjectWorktreeCapability['adopt']>[0]) => {
			const currentCapability = capability;
			if (currentCapability === undefined || activeRequest.current !== null || disposed.current) {
				return;
			}
			const completeActivation = beginWorkspaceActivation();
			activeRequest.current = { kind: 'adoption', sourceWorkspaceId: input.sourceWorkspaceId };
			setBusy(true);
			setFailure(null);
			void currentCapability
				.adopt(input)
				.catch(() => ({ code: 'control-unavailable' as const, kind: 'failed' as const }))
				.then((result) => {
					if (disposed.current) return;
					if (result.kind === 'failed') setFailure(result.code);
					else completeActivation(result.workspaceId);
				})
				.finally(() => {
					if (disposed.current) return;
					activeRequest.current = null;
					const nextWorkspaceId = queuedWorkspaceId.current;
					queuedWorkspaceId.current = null;
					if (nextWorkspaceId === null) {
						setBusy(false);
					} else {
						startDiscovery(nextWorkspaceId);
					}
				});
		},
	);

	return {
		adopt,
		busy,
		discoveryByWorkspace,
		discoveryFailureWorkspaceIds,
		dismissFailure: useMemoizedFn(() => setFailure(null)),
		failure,
		refresh,
	};
}
