import type { ActionExecution, ActionResult } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchWorktreeOpening } from '../../../actions/workbench-action-ids';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../types/workbench-action-menu';
import type {
	WorkbenchWorktreeDiscovery,
	WorkbenchWorktreeDiscoveryResult,
} from '../../../types/workbench-worktrees';
import type { WorkbenchWorktreeCommands } from '../types/workbench-sidebar-types';
import { worktreeRowsShown, type WorktreeRowsEvidence } from '../utils/worktreeRowsShown';
import type { WorkbenchSidebarWorkspaceLocation } from './useWorkbenchSidebarWorkspaceLocations';

type WorkspaceDiscovery = { readonly kind: 'reading' } | WorkbenchWorktreeDiscoveryResult;

/** The current read of a Workspace's repository, and what the last settled one decided. */
interface WorkspaceWorktreeState {
	readonly current: WorkspaceDiscovery;
	readonly settled: WorktreeRowsEvidence['settled'];
}

export interface WorkspaceWorktreeMenu {
	/** The destructive command that follows Close, when the Workspace may remove its checkout. */
	readonly removal: readonly WorkbenchActionMenuEntry[];
	/** The separated Worktree section. */
	readonly section: readonly WorkbenchActionMenuEntry[];
}

/** What a Project's New Workspace submenu offers from the repository of its first Workspace. */
export interface ProjectWorktreeRows {
	/** The Git Worktrees group: the repository's other checkouts, once Git answers. */
	readonly checkouts: readonly WorkbenchActionMenuCommand[];
	/** New Worktree, or null where the authority cannot reach Git. */
	readonly create: WorkbenchActionMenuCommand | null;
}

export interface WorkspaceWorktreesView {
	readonly menu: (workspaceId: string) => WorkspaceWorktreeMenu;
	/** For a Project whose first Workspace is the one named, or for a Project with none. */
	readonly projectRows: (sourceWorkspaceId: string | null) => ProjectWorktreeRows;
	/** Reads the Workspace's repository again; a menu calls it each time it opens. */
	readonly refresh: (workspaceId: string) => void;
}

interface UseWorkspaceWorktreesOptions {
	readonly beginWorkspaceActivation: () => (workspaceId: string) => void;
	readonly commands: WorkbenchWorktreeCommands | undefined;
	/** What each Workspace row shows of where it stands, including the branch Git reported there. */
	readonly locations: ReadonlyMap<string, WorkbenchSidebarWorkspaceLocation>;
}

const NO_MENU: WorkspaceWorktreeMenu = { removal: [], section: [] };
const NO_PROJECT_ROWS: ProjectWorktreeRows = { checkouts: [], create: null };

/**
 * The Worktree rows of Project and Workspace menus, as the TUI offers them: discovery starts when the
 * menu opens and the rows stay disabled until Git answers. Rows show only where Git is known, from
 * the row's branch or a discovery that found Git, so a Workspace outside Git never shows them. A
 * Project reads the repository of its first Workspace. The rows invoke Workbench Actions, which own
 * the decisions; while one runs, the rows wait for it.
 */
export function useWorkspaceWorktrees({
	beginWorkspaceActivation,
	commands,
	locations,
}: UseWorkspaceWorktreesOptions): WorkspaceWorktreesView {
	const intl = useOrchbeeIntl();
	const [discoveries, setDiscoveries] = React.useState<ReadonlyMap<string, WorkspaceWorktreeState>>(
		() => new Map(),
	);
	const [pending, setPending] = React.useState(false);
	const generations = React.useRef(new Map<string, number>());
	const mounted = React.useRef(true);

	React.useEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
		};
	}, []);

	const refresh = useMemoizedFn((workspaceId: string) => {
		if (commands === undefined) return;
		// Only the newest read speaks for the Workspace.
		const generation = (generations.current.get(workspaceId) ?? 0) + 1;
		generations.current.set(workspaceId, generation);
		// The answer speaks for what the row shows as the read begins.
		const line = locations.get(workspaceId)?.line ?? null;
		setDiscoveries((current) =>
			new Map(current).set(workspaceId, {
				current: { kind: 'reading' },
				settled: current.get(workspaceId)?.settled ?? null,
			}),
		);
		void commands
			.discover(workspaceId)
			.catch(
				(): WorkbenchWorktreeDiscoveryResult => ({
					kind: 'failed',
					reason: 'unavailable',
					stoppedRemoval: null,
				}),
			)
			.then((result) => {
				if (!mounted.current || generations.current.get(workspaceId) !== generation) return;
				setDiscoveries((current) =>
					new Map(current).set(workspaceId, {
						current: result,
						settled: { git: result.kind === 'discovered', line },
					}),
				);
			});
	});

	const follow = <Output>(
		start: () => ActionExecution<Output>,
		settle: (result: ActionResult<Output>) => void,
	): void => {
		setPending(true);
		void start()
			.completion.then((result) => {
				if (mounted.current) settle(result);
			})
			.finally(() => {
				if (mounted.current) setPending(false);
			});
	};

	/** A Worktree that opens becomes where the person is, unless they have moved on meanwhile. */
	const openInto = (start: () => ActionExecution<WorkbenchWorktreeOpening>): void => {
		const completeActivation = beginWorkspaceActivation();
		follow(start, (result) => {
			if (result.kind === 'completed' && result.value !== undefined) {
				completeActivation(result.value.workspaceId);
			}
		});
	};

	const discoveryOf = (workspaceId: string): WorkspaceDiscovery =>
		discoveries.get(workspaceId)?.current ?? { kind: 'reading' };

	const rowsShown = (workspaceId: string): boolean =>
		worktreeRowsShown({
			line: locations.get(workspaceId)?.line ?? null,
			settled: discoveries.get(workspaceId)?.settled ?? null,
		});

	const newWorktreeRow = (
		available: WorkbenchWorktreeCommands,
		workspaceId: string,
		discovery: WorkbenchWorktreeDiscovery | null,
	): WorkbenchActionMenuCommand => ({
		disabled: discovery === null || pending,
		icon: 'git-branch',
		id: 'new-worktree',
		kind: 'command',
		label: intl.formatMessage(workbenchMessages.worktreeNew),
		onSelect: () => openInto(() => available.create(workspaceId)),
	});

	const checkoutRows = (
		available: WorkbenchWorktreeCommands,
		workspaceId: string,
		discovery: WorkbenchWorktreeDiscovery,
	): readonly WorkbenchActionMenuCommand[] =>
		discovery.checkouts.map((checkout, index) => ({
			disabled: pending,
			icon: 'git-branch',
			id: `worktree-checkout-${index}`,
			kind: 'command',
			label: checkout.branch ?? checkout.path,
			onSelect: () => openInto(() => available.open({ path: checkout.path, workspaceId })),
		}));

	const projectRows = (sourceWorkspaceId: string | null): ProjectWorktreeRows => {
		// A Project without Workspaces has no source.
		if (commands === undefined || sourceWorkspaceId === null || !rowsShown(sourceWorkspaceId)) {
			return NO_PROJECT_ROWS;
		}
		const state = discoveryOf(sourceWorkspaceId);
		const discovery = state.kind === 'discovered' ? state.discovery : null;
		return {
			checkouts: discovery === null ? [] : checkoutRows(commands, sourceWorkspaceId, discovery),
			create: newWorktreeRow(commands, sourceWorkspaceId, discovery),
		};
	};

	const menu = (workspaceId: string): WorkspaceWorktreeMenu => {
		if (commands === undefined) return NO_MENU;
		const state = discoveryOf(workspaceId);
		const discovery = state.kind === 'discovered' ? state.discovery : null;
		const status = (() => {
			if (state.kind === 'reading') return workbenchMessages.worktreeReading;
			if (state.kind === 'failed') {
				return state.reason === 'not-repository'
					? workbenchMessages.worktreeNotRepository
					: workbenchMessages.worktreeListingUnavailable;
			}
			return workbenchMessages.worktreeNoOtherCheckouts;
		})();
		const checkouts: readonly WorkbenchActionMenuCommand[] =
			discovery === null || discovery.checkouts.length === 0
				? [
						{
							disabled: true,
							icon: 'git-branch',
							id: 'worktree-status',
							kind: 'command',
							label: intl.formatMessage(status),
							onSelect: () => undefined,
						},
					]
				: checkoutRows(commands, workspaceId, discovery);
		/**
		 * Remove Worktree… is the one removal entry. Where a removal of this Workspace stopped, it
		 * continues that removal, since a new one beside it is not proven safe.
		 */
		const removalEntries = (): readonly WorkbenchActionMenuEntry[] => {
			const stopped = state.kind === 'reading' ? null : state.stoppedRemoval;
			if (stopped !== null) {
				return [
					{
						destructive: true,
						disabled: pending,
						icon: 'trash',
						id: 'remove-worktree',
						kind: 'command',
						label: intl.formatMessage(workbenchMessages.worktreeRemove),
						onSelect: () =>
							follow(
								() => commands.resume({ cause: stopped.cause, operationId: stopped.operationId }),
								() => undefined,
							),
					},
				];
			}
			const removal = discovery?.removal ?? null;
			if (removal === null) return [];
			return [
				{
					destructive: true,
					disabled: pending,
					icon: 'trash',
					id: 'remove-worktree',
					kind: 'command',
					label: intl.formatMessage(workbenchMessages.worktreeRemove),
					onSelect: () =>
						follow(
							() =>
								commands.remove({
									branch: removal.branch,
									path: removal.path,
									terminalCount: removal.terminalCount,
									workspaceId,
								}),
							() => undefined,
						),
				},
			];
		};
		if (!rowsShown(workspaceId)) return { removal: removalEntries(), section: [] };
		return {
			removal: removalEntries(),
			section: [
				{ id: 'worktree-separator', kind: 'separator' },
				newWorktreeRow(commands, workspaceId, discovery),
				{
					groups: [{ commands: checkouts, id: 'worktree-checkouts' }],
					icon: 'git-branch',
					id: 'open-worktree',
					kind: 'submenu',
					label: intl.formatMessage(workbenchMessages.worktreeOpen),
				},
			],
		};
	};

	return { menu, projectRows, refresh };
}
