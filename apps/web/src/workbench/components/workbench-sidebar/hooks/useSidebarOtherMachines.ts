import * as React from 'react';

import type { WorkbenchProject } from '../../../../project-workspace';
import type {
	WorkbenchMachineMark,
	WorkbenchOtherMachines,
	WorkbenchOtherMachinesSnapshot,
} from '../../../types/workbench-other-machines';
import type { SidebarOtherMachineRow } from '../components/project-navigation-item/ProjectNavigationItem';

const NO_MACHINES: WorkbenchOtherMachinesSnapshot = { card: null, machines: [], onScreen: null };
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
): SidebarOtherMachinesView {
	const others = React.useSyncExternalStore(
		otherMachines?.subscribe ?? NO_SUBSCRIPTION,
		otherMachines?.getSnapshot ?? NO_SNAPSHOT,
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
	return {
		foreignExpanded: (projectId) => !collapsedForeign.has(projectId),
		groups: merged,
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
