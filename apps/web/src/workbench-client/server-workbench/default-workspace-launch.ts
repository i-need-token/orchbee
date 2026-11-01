import { parseWorkbenchAction, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import type { ServerWorkbenchAction } from './ServerWorkbenchSession';

const MAXIMUM_WORKSPACE_LABEL_SCALARS = 512;

/** Every surface resizes its terminal once attached, so this only has to be a usable first size. */
const DEFAULT_TERMINAL_GEOMETRY = {
	cellPixelHeight: 18,
	cellPixelWidth: 9,
	columns: 120,
	rows: 30,
};

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

/** One ungrouped Workspace with its first default shell, atomically, as every Client makes it. */
export function defaultWorkspaceLaunch(
	directory: string,
	fallbackLabel: string,
): ServerWorkbenchAction {
	return parseWorkbenchAction({
		automaticLabel: true,
		geometry: DEFAULT_TERMINAL_GEOMETRY,
		kind: 'workspace.launch',
		label: workspaceLabel(directory, fallbackLabel),
		options: { cwd: directory },
		placement: { kind: 'ungrouped' },
	});
}
