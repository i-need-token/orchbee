import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import { readServerWorkbenchAction } from './readServerWorkbenchAction';
import type { ServerWorkbenchAction } from './ServerWorkbenchSession';

const MAXIMUM_WORKSPACE_LABEL_SCALARS = 512;

/** Every surface resizes its terminal once attached, so this only has to be a usable first size. */
const DEFAULT_TERMINAL_GEOMETRY = {
	cellPixelHeight: 18,
	cellPixelWidth: 9,
	columns: 120,
	rows: 30,
};

export type ServerWorkspacePlacement =
	| { readonly kind: 'ungrouped' }
	| { readonly kind: 'project'; readonly projectId: string }
	| { readonly kind: 'new-project'; readonly label: string };

interface WorkspaceLaunchRequest {
	readonly directory: string;
	/** Names a directory that has no name of its own, such as a drive root. */
	readonly fallbackLabel: string;
	readonly placement: ServerWorkspacePlacement;
	/** The snapshot the command will name, so a generated label avoids what it can see. */
	readonly snapshot: WorkbenchSnapshot;
}

/**
 * The rule the TUI starts by: a Session holding no Workspace and no terminal may open one. A Workspace
 * with no Tabs still counts, because someone left it that way.
 */
export function admitsAutomaticWorkspace(snapshot: WorkbenchSnapshot): boolean {
	return (
		snapshot.navigation.workspaces.length === 0 &&
		snapshot.terminalContexts.every((context) => context.tabs.length === 0)
	);
}

/** A directory's name labels it, and a root has none, so the localized fallback does. */
function workspaceLabel(directory: string, fallbackLabel: string): string {
	const name =
		directory
			.replace(/[\\/]+$/u, '')
			.split(/[\\/]/u)
			.at(-1) ?? '';
	const label = name.trim().length === 0 || name.endsWith(':') ? fallbackLabel : name;
	return [...label].slice(0, MAXIMUM_WORKSPACE_LABEL_SCALARS).join('');
}

/**
 * Avoids every label this snapshot holds, as the TUI does. An automatic Workspace is named by the
 * same rule once its first directory is sampled, so choosing otherwise would visibly rename it.
 */
function generatedWorkspaceLabel(
	directory: string,
	fallbackLabel: string,
	snapshot: WorkbenchSnapshot,
): string {
	const name = workspaceLabel(directory, fallbackLabel);
	const taken = new Set(snapshot.navigation.workspaces.map((workspace) => workspace.label));
	let label = name;
	for (let ordinal = 2; taken.has(label); ordinal += 1) {
		const suffix = ` (${ordinal})`;
		label = `${[...name].slice(0, MAXIMUM_WORKSPACE_LABEL_SCALARS - suffix.length).join('')}${suffix}`;
	}
	return label;
}

/** One Workspace with its first default shell, atomically, as every Client makes it. */
export function workspaceLaunchAction({
	directory,
	fallbackLabel,
	placement,
	snapshot,
}: WorkspaceLaunchRequest): ServerWorkbenchAction | null {
	return readServerWorkbenchAction({
		automaticLabel: true,
		geometry: DEFAULT_TERMINAL_GEOMETRY,
		kind: 'workspace.launch',
		label: generatedWorkspaceLabel(directory, fallbackLabel, snapshot),
		options: { cwd: directory },
		placement,
	});
}
