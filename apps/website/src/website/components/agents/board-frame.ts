// The Agent board (Ctrl+B g) over the session: the product's 92-cell overlay, its Start Agent row,
// one row per Agent in the product's "status · title · Agent · Workspace" form, the key hints and
// the footer actions. `selected` is the highlighted row.
import {
	createGrid,
	paintPanel,
	paintSidebar,
	paintTabStrip,
	type AgentState,
	type TuiLine,
} from '../../../tui-grid';

const BOARD_COLUMNS = 104;

interface BoardAgent {
	readonly state: AgentState;
	readonly status: string;
	readonly title: string;
	readonly kind: string;
	readonly workspace: string;
	readonly unread: boolean;
}

export const BOARD_AGENTS: readonly BoardAgent[] = [
	{
		state: 'attention',
		status: 'needs attention · 1',
		title: 'claude-1',
		kind: 'Claude Code',
		workspace: 'website',
		unread: false,
	},
	{
		state: 'working',
		status: 'working',
		title: 'reviewer',
		kind: 'Codex',
		workspace: 'website',
		unread: false,
	},
	{
		state: 'done',
		status: 'Done',
		title: 'docs',
		kind: 'OpenCode',
		workspace: 'website',
		unread: true,
	},
	{
		state: 'failed',
		status: 'failed',
		title: 'api-fix',
		kind: 'Claude Code',
		workspace: 'api-server',
		unread: true,
	},
	{
		state: 'idle',
		status: 'idle',
		title: 'sweeper',
		kind: 'Gemini CLI',
		workspace: 'design-tokens',
		unread: false,
	},
];

export function boardFrame(selected: number): TuiLine[] {
	const width = BOARD_COLUMNS;
	const height = 22;
	const side = 32;
	const grid = createGrid(width, height);
	paintSidebar(grid, side, height, {
		project: 'orchbee',
		workspaces: [
			{
				name: 'website',
				branch: 'feat/website',
				selected: true,
				ahead: '↑3',
				agents: [
					{ state: 'attention', title: 'claude-1', kind: 'Claude Code' },
					{ state: 'working', title: 'reviewer', kind: 'Codex' },
					{ state: 'done', title: 'docs', kind: 'OpenCode', unread: true },
				],
			},
			{
				name: 'api-server',
				branch: 'fix/rate-limit',
				agents: [{ state: 'failed', title: 'api-fix', kind: 'Claude Code', unread: true }],
			},
			{
				name: 'design-tokens',
				branch: 'main',
				agents: [{ state: 'idle', title: 'sweeper', kind: 'Gemini CLI' }],
			},
		],
	});
	paintTabStrip(grid, 0, side + 1, [
		{ title: 'build', state: 'attention', active: true },
		{ title: 'docs', state: 'done', unread: true },
	]);
	paintPanel(grid, {
		top: 1,
		left: side + 1,
		bottom: height - 1,
		right: width - 1,
		title: 'claude-1',
		state: 'attention',
		focused: true,
	});
	grid.run(3, side + 3, [
		['> ', 'focus'],
		['Add the install section with OS tabs', 'primary', { bold: true }],
	]);
	grid.run(17, side + 3, [['● ', 'secondary'], ['Run  '], ['pnpm test', 'secondary']]);
	grid.run(18, side + 5, [
		['14 passed', 'success'],
		['  1.2s', 'secondary'],
	]);
	grid.run(20, side + 3, [['? ', 'warning'], ['Run this command?  git push origin feat/website']]);

	// The overlay is centered over the whole interface, as the product places it.
	const left = 6;
	const right = left + 91;
	const top = 5;
	const bottom = top + 10;
	for (let row = top; row <= bottom; row += 1) grid.clear(row, left, right, 'overlay');
	grid.frame(top, left, bottom, right, 'unfocused', 'square');
	const surface = { surface: 'overlay' } as const;
	grid.put(top, left + 2, ' ', 'primary', surface);
	const after = grid.put(top, left + 3, 'Agent board', 'primary', { ...surface, bold: true });
	grid.put(top, after, ' ', 'primary', surface);
	grid.put(top, right - 6, ' ', 'primary', surface);
	grid.put(top, right - 5, '[x]', 'secondary', surface);
	grid.put(top, right - 2, ' ', 'primary', surface);
	const inner = left + 3;
	const end = right - 3;
	grid.put(top + 1, inner + 1, '+ Start Agent', 'primary', surface);
	BOARD_AGENTS.forEach((agent, index) => {
		const row = top + 2 + index;
		const on = index === selected;
		const rowSurface = { surface: on ? 'selected' : 'overlay' } as const;
		if (on) {
			grid.fill(row, inner, end, 'selected');
			grid.put(row, inner, '›', 'focus', rowSurface);
		}
		grid.glyph(row, inner + 1, agent.state, rowSurface.surface);
		grid.put(
			row,
			inner + 3,
			[agent.status, agent.title, agent.kind, agent.workspace].join(' · '),
			'primary',
			rowSurface,
		);
		if (agent.unread) grid.put(row, end, '•', 'unread', rowSurface);
	});
	const hints: readonly (readonly [string, string])[] = [
		['Esc', 'close'],
		['Enter', 'jump'],
		['s', 'start Agent'],
		['r', 'refresh'],
		['↑/k, ↓/j', 'select'],
	];
	let next = inner;
	hints.forEach(([key, label], index) => {
		if (index > 0)
			next = grid.run(top + 8, next, [
				['  ', 'secondary', surface],
				['│', 'divider', surface],
				['  ', 'secondary', surface],
			]);
		next = grid.run(top + 8, next, [
			[key, 'primary', { ...surface, bold: true }],
			[` ${label}`, 'secondary', surface],
		]);
	});
	grid.run(top + 9, inner, [
		['[ Open ]', 'focus', { ...surface, bold: true }],
		['  ', 'secondary', surface],
		['[ Refresh ]', 'secondary', surface],
	]);
	return grid.lines();
}
