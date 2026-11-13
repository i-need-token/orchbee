// The parts of the Orchbee terminal interface every scene shares, painted the way the product
// paints them: the sidebar with its header and footer controls, the Tab strip, and Panel frames
// whose title sits on the top edge.
import type { AgentState, TuiGrid, TuiPutOptions, TuiSurface } from './createGrid';

interface SidebarAgent {
	readonly state: AgentState;
	readonly title: string;
	readonly kind?: string;
	readonly unread?: boolean;
}

interface SidebarWorkspace {
	readonly name: string;
	readonly branch: string;
	readonly selected?: boolean;
	readonly ahead?: string;
	readonly agents?: readonly SidebarAgent[];
}

interface SidebarContent {
	readonly project: string;
	readonly workspaces: readonly SidebarWorkspace[];
}

/** Paints a sidebar `width` cells wide and the divider column right after it. */
export function paintSidebar(
	grid: TuiGrid,
	width: number,
	rows: number,
	content: SidebarContent,
): void {
	const last = width - 2;
	for (let row = 0; row < rows; row += 1) grid.put(row, width, '│', 'divider');
	grid.put(0, 0, '[ New workspace ]');
	grid.right(0, last, '[«]', 'secondary');
	grid.put(2, 1, `${content.project} ▾`, 'secondary');
	let row = 3;
	for (const workspace of content.workspaces) {
		const surface: TuiSurface | undefined = workspace.selected === true ? 'current' : undefined;
		const options: TuiPutOptions = surface === undefined ? {} : { surface };
		if (surface !== undefined) {
			grid.fill(row, 0, width - 1, surface);
			grid.fill(row + 1, 0, width - 1, surface);
		}
		grid.put(row, 1, `${workspace.name} ▾`, 'primary', options);
		if (workspace.ahead !== undefined) grid.right(row, last, workspace.ahead, 'secondary', options);
		grid.put(row + 1, 1, workspace.branch, 'secondary', options);
		row += 2;
		for (const agent of workspace.agents ?? []) {
			grid.glyph(row, 4, agent.state);
			const budget = last - 1 - 6;
			const [title, kind] = agentRowText(agent.title, agent.kind, budget);
			const next = grid.put(row, 6, title);
			if (kind !== '') grid.put(row, next, kind, 'secondary');
			if (agent.unread === true) grid.put(row, last, '•', 'unread');
			row += 1;
		}
		row += 1;
	}
	grid.put(rows - 1, 0, '[ Settings ]');
	grid.right(rows - 1, last, '[?]', 'secondary');
}

// The row keeps the Agent kind and shortens the title first, as the sidebar does.
function agentRowText(title: string, kind: string | undefined, budget: number): [string, string] {
	const suffix = kind === undefined ? '' : ` · ${kind}`;
	if (title.length + suffix.length <= budget) return [title, suffix];
	const room = budget - suffix.length;
	if (room >= 4) return [`${title.slice(0, room - 1)}…`, suffix];
	const whole = `${title}${suffix}`;
	return [`${whole.slice(0, budget - 1)}…`, ''];
}

interface StripTab {
	readonly title: string;
	readonly state?: AgentState;
	readonly active?: boolean;
	readonly unread?: boolean;
}

/** Paints the Tab strip from `col`: each Tab's glyph, title and unread mark, then `[+]`. */
export function paintTabStrip(
	grid: TuiGrid,
	row: number,
	col: number,
	tabs: readonly StripTab[],
): void {
	let next = col + 1;
	for (const tab of tabs) {
		if (tab.state !== undefined) next = grid.glyph(row, next, tab.state) + 1;
		next = grid.put(row, next, tab.title, tab.active === true ? 'primary' : 'secondary', {
			bold: tab.active === true,
		});
		if (tab.unread === true) next = grid.put(row, next + 1, '•', 'unread');
		if (tab.active === true) next = grid.put(row, next + 1, '[x]', 'secondary');
		next += 2;
	}
	grid.put(row, next, '[+]', 'secondary');
}

interface PanelFrame {
	readonly top: number;
	readonly left: number;
	readonly bottom: number;
	readonly right: number;
	readonly title: string;
	readonly focused: boolean;
	readonly state?: AgentState;
	readonly unread?: boolean;
}

/** Paints a Panel frame with its title on the top edge, `┌─ ⬢ title •`. */
export function paintPanel(grid: TuiGrid, panel: PanelFrame): void {
	grid.frame(
		panel.top,
		panel.left,
		panel.bottom,
		panel.right,
		panel.focused ? 'focus' : 'unfocused',
		'square',
	);
	let next = grid.put(panel.top, panel.left + 2, ' ');
	if (panel.state !== undefined) next = grid.glyph(panel.top, next, panel.state) + 1;
	next = grid.put(panel.top, next, panel.title, panel.focused ? 'focus' : 'secondary', {
		bold: panel.focused,
	});
	if (panel.unread === true) next = grid.put(panel.top, next + 1, '•', 'unread');
	grid.put(panel.top, next, ' ');
}
