// The orchestrate session at view 0 alone, 1 Codex started, 2 Codex reviewing, 3 Codex done and
// unread, 4 its answer read back. claude-1 runs the same orchbee commands as the shell beside it.
import {
	createGrid,
	paintPanel,
	paintSidebar,
	paintTabStrip,
	type AgentState,
	type TuiLine,
} from '../../../tui-grid';

export type OrchestrateView = 0 | 1 | 2 | 3 | 4;

const ORCHESTRATE_COLUMNS = 92;

const ELAPSED = ['0m 03s', '0m 09s', '0m 21s', '1m 02s', '1m 09s'] as const;

export function orchestrateFrame(view: OrchestrateView): TuiLine[] {
	const width = ORCHESTRATE_COLUMNS;
	const height = 18;
	const side = 26;
	const split = 58;
	const grid = createGrid(width, height);
	let codex: AgentState = 'done';
	if (view === 1) codex = 'idle';
	else if (view === 2) codex = 'working';
	paintSidebar(grid, side, height, {
		project: 'orchbee',
		workspaces: [
			{
				name: 'website',
				branch: 'feat/website',
				selected: true,
				agents: [
					{ state: 'working', title: 'claude-1', kind: 'Claude Code' },
					...(view >= 1 ? [{ state: codex, title: 'Codex', unread: view === 3 }] : []),
				],
			},
			{
				name: 'api-server',
				branch: 'fix/rate-limit',
				agents: [{ state: 'failed', title: 'api-fix', kind: 'Claude Code', unread: true }],
			},
		],
	});
	paintTabStrip(grid, 0, side + 1, [{ title: 'build', state: 'working', active: true }]);
	const main = side + 1;
	paintPanel(grid, {
		top: 1,
		left: main,
		bottom: height - 1,
		right: view === 0 ? width - 1 : split,
		title: 'claude-1',
		state: 'working',
		focused: view === 0 || view === 4,
	});
	if (view >= 1) {
		paintPanel(grid, {
			top: 1,
			left: split + 1,
			bottom: height - 1,
			right: width - 1,
			title: 'Codex',
			state: codex,
			unread: view === 3,
			focused: view < 4,
		});
	}

	const left = main + 2;
	grid.run(3, left, [
		['> ', 'focus'],
		['Ask Codex for a review', 'primary', { bold: true }],
	]);
	const command = (row: number, verb: string, rest: string): void => {
		grid.run(row, left, [['● ', 'secondary'], [`orchbee agent ${verb}`]]);
		grid.put(row + 1, left + 2, rest, 'secondary');
	};
	if (view >= 1) command(5, 'start', 'codex --split right');
	if (view >= 2) {
		command(7, 'prompt', '@panel:0x36 "Review…"');
		command(9, 'wait', '@panel:0x36 --until idle');
	}
	if (view >= 4) {
		command(11, 'read', '@panel:0x36 --lines 3');
		grid.put(14, left, 'Two notes. Fixing both.');
	}
	grid.glyph(16, left, 'working');
	grid.run(16, left + 1, [[' Working'], [`  ${ELAPSED[view]}`, 'secondary']]);

	const review = split + 3;
	if (view === 1) grid.run(3, review, [['> ', 'focus'], ['█']]);
	if (view >= 2) {
		grid.run(3, review, [
			['> ', 'focus'],
			['Review feat/website', 'primary', { bold: true }],
		]);
		grid.run(5, review, [['● ', 'secondary'], ['git diff --stat']]);
	}
	if (view === 2) {
		grid.glyph(7, review, 'working');
		grid.run(7, review + 1, [[' Working'], ['  0m 12s', 'secondary']]);
	}
	if (view >= 3) {
		grid.run(6, review, [['● ', 'secondary'], ['Checked tab order']]);
		grid.put(8, review, 'No blocking issues.');
		grid.put(9, review, 'Two notes:');
		grid.put(10, review, '1. Copy button and Tab', 'secondary');
		grid.put(11, review, '2. zh-CN wrap at 320px', 'secondary');
	}
	return grid.lines();
}
