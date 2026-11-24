import * as React from 'react';

import type { WorkbenchProject } from '../../../../project-workspace';
import type { WorkbenchActionMenuCommand } from '../../../types/workbench-action-menu';
import type {
	WorkbenchMachineChangeRow,
	WorkbenchMachineMark,
	WorkbenchOtherMachines,
	WorkbenchOtherMachinesSnapshot,
} from '../../../types/workbench-other-machines';
import type { SidebarOtherMachineRow } from '../components/project-navigation-item/ProjectNavigationItem';
import type { WorkspaceMachineMenu } from '../types/workbench-sidebar-types';

const NO_MACHINES: WorkbenchOtherMachinesSnapshot = {
	card: null,
	change: null,
	machines: [],
	onScreen: null,
};
/** Past this many saved machines, Change machine's list leads with a query. */
const LISTED_MACHINES = 8;
const NO_SUBSCRIPTION = (): (() => void) => () => undefined;
const NO_SNAPSHOT = (): WorkbenchOtherMachinesSnapshot => NO_MACHINES;

/** One group of the merged sidebar: a Project, or Recents (null). */
interface SidebarGroup {
	/** A Project only other machines hold. */
	readonly foreign: boolean;
	readonly project: WorkbenchProject | null;
}

interface SidebarOtherMachinesView {
	readonly groups: readonly SidebarGroup[];
	readonly machineMenu: WorkspaceMachineMenu | undefined;
	readonly onScreenMark: WorkbenchMachineMark | null;
	readonly open: (machine: string, workspaceId: string) => void;
	readonly otherRows: (projectId: string | null) => {
		readonly leading: readonly SidebarOtherMachineRow[];
		readonly trailing: readonly SidebarOtherMachineRow[];
	};
	/** Whether a foreign Project's group is open; folding it is this sidebar's own. */
	readonly foreignExpanded: (projectId: string) => boolean;
	readonly toggleForeign: (projectId: string) => void;
}

/**
 * Merges other machines' Workspaces into the groups of the Session on screen, as the TUI's sidebar
 * does: Projects merge by ID, a Project only other machines hold follows the Session's own, and
 * one Recents group closes the list while any machine has a Workspace outside a Project.
 */
export function useSidebarOtherMachines(
	otherMachines: WorkbenchOtherMachines | undefined,
	projects: readonly WorkbenchProject[],
	hasUngrouped: boolean,
	onManageMachines: (() => void) | undefined,
): SidebarOtherMachinesView {
	const read = otherMachines?.getSnapshot ?? NO_SNAPSHOT;
	const others = React.useSyncExternalStore(
		otherMachines?.subscribe ?? NO_SUBSCRIPTION,
		read,
		read,
	);
	const [collapsedForeign, setCollapsedForeign] = React.useState<ReadonlySet<string>>(new Set());
	const merged = React.useMemo(() => {
		const known = new Set(projects.map((project) => project.id));
		const foreign: WorkbenchProject[] = [];
		let ungrouped = hasUngrouped;
		for (const machine of others.machines) {
			for (const project of machine.projects) {
				if (known.has(project.id)) continue;
				known.add(project.id);
				foreign.push({ folders: [], id: project.id, name: project.name });
			}
			ungrouped ||= machine.workspaces.some((workspace) => workspace.projectId === null);
		}
		const groups: SidebarGroup[] = [
			...projects.map((project) => ({ foreign: false, project })),
			...foreign.map((project) => ({ foreign: true, project })),
		];
		if (ungrouped) groups.push({ foreign: false, project: null });
		return groups;
	}, [hasUngrouped, others, projects]);
	const otherRows = React.useMemo(
		() => (projectId: string | null) => {
			const rows = (leading: boolean): SidebarOtherMachineRow[] =>
				others.machines
					.filter((machine) => machine.leading === leading)
					.flatMap((machine) =>
						machine.workspaces
							.filter((workspace) => workspace.projectId === projectId)
							.map((workspace) => ({
								machine: machine.machine,
								mark: machine.mark,
								name: workspace.name,
								word: machine.word,
								workspaceId: workspace.id,
							})),
					);
			return { leading: rows(true), trailing: rows(false) };
		},
		[others],
	);
	// The list's order as it opened, so a row never moves under the pointer.
	const listOrder = React.useRef<readonly string[]>([]);
	const change = others.change;
	const machineMenu: WorkspaceMachineMenu | undefined =
		otherMachines === undefined || change === null
			? undefined
			: {
					opened: (open) => {
						if (open) listOrder.current = change.rows.map((row) => row.machine);
						otherMachines.watchCapacity(open);
					},
					submenu: (workspace) => {
						const place = (row: WorkbenchMachineChangeRow): number => {
							const index = listOrder.current.indexOf(row.machine);
							return index === -1 ? listOrder.current.length : index;
						};
						const rows = [...change.rows].sort((left, right) => place(left) - place(right));
						const command = (row: WorkbenchMachineChangeRow): WorkbenchActionMenuCommand => ({
							disabled: change.moving || row.disabled,
							icon: row.current ? 'check' : 'machine',
							id: `machine-${row.machine}`,
							kind: 'command',
							label: row.name,
							onSelect: () => {
								if (!row.current) otherMachines.move(workspace.id, workspace.name, row.machine);
							},
							shortcut: row.trailing,
						});
						const manage: WorkbenchActionMenuCommand[] =
							onManageMachines === undefined
								? []
								: [
										{
											icon: 'settings',
											id: 'manage-machines',
											kind: 'command',
											label: change.labels.manage,
											onSelect: onManageMachines,
										},
									];
						if (rows.length - 1 > LISTED_MACHINES) {
							return {
								commands: (query) => [
									...rows.filter((row) => matches(row, query)).map(command),
									...manage,
								],
								emptyLabel: () => change.labels.noMatch,
								icon: 'machine',
								id: 'workspace-machine',
								kind: 'search-submenu',
								label: change.labels.title,
								searchLabel: change.labels.find,
							};
						}
						return {
							groups: [
								{
									commands: rows.map(command),
									id: 'machines',
									label: `${change.labels.heading} · ${change.labels.note}`,
								},
								...(manage.length === 0 ? [] : [{ commands: manage, id: 'manage' }]),
							],
							icon: 'machine',
							id: 'workspace-machine',
							kind: 'submenu',
							label: change.labels.title,
						};
					},
				};
	return {
		foreignExpanded: (projectId) => !collapsedForeign.has(projectId),
		groups: merged,
		machineMenu,
		onScreenMark: others.onScreen,
		open: (machine, workspaceId) => otherMachines?.open(machine, workspaceId),
		otherRows,
		toggleForeign: (projectId) =>
			setCollapsedForeign((collapsed) => {
				const next = new Set(collapsed);
				if (!next.delete(projectId)) next.add(projectId);
				return next;
			}),
	};
}

/** Whether `query` names `row`: any part of its name or address, without regard to case. */
function matches(row: WorkbenchMachineChangeRow, query: string): boolean {
	const wanted = query.trim().toLowerCase();
	return (
		wanted === '' ||
		row.name.toLowerCase().includes(wanted) ||
		(row.address?.toLowerCase().includes(wanted) ?? false)
	);
}
