// The hero session in four phases: 0 editing, 1 testing, 2 summarizing while the reviewer is done,
// 3 waiting for approval. Everything inside the Panels is the Agents' own output.
import {
	createGrid,
	paintPanel,
	paintSidebar,
	paintTabStrip,
	type AgentState,
	type TuiGrid,
	type TuiLine,
} from '../../../tui-grid';

export type HeroPhase = 0 | 1 | 2 | 3;

const HERO_COLUMNS = 148;
const COMPACT_HERO_COLUMNS = 54;

function working(grid: TuiGrid, row: number, col: number, detail: string): void {
	grid.glyph(row, col, 'working');
	grid.run(row, col + 1, [[' Working'], [`  ${detail}`, 'secondary']]);
}

function step(grid: TuiGrid, row: number, col: number, verb: string, target: string): void {
	grid.run(row, col, [['● ', 'secondary'], [verb], [target, 'secondary']]);
}

export function heroFrame(phase: HeroPhase): TuiLine[] {
	const width = HERO_COLUMNS;
	const height = 36;
	const side = 32;
	const grid = createGrid(width, height);
	const claudeState: AgentState = phase < 3 ? 'working' : 'attention';
	const reviewer: AgentState = phase < 2 ? 'working' : 'done';
	paintSidebar(grid, side, height, {
		project: 'orchbee',
		workspaces: [
			{
				name: 'website',
				branch: 'feat/website',
				selected: true,
				ahead: '↑3',
				agents: [
					{ state: claudeState, title: 'claude-1', kind: 'Claude Code' },
					{ state: reviewer, title: 'reviewer', kind: 'Codex', unread: phase >= 2 },
					{ state: 'done', title: 'docs', kind: 'OpenCode', unread: true },
				],
			},
			{
				name: 'api-server',
				branch: 'fix/rate-limit',
				ahead: '↓1',
				agents: [
					{ state: 'failed', title: 'api-fix', kind: 'Claude Code', unread: true },
					{ state: 'idle', title: 'sweeper', kind: 'Gemini CLI' },
				],
			},
			{ name: 'design-tokens', branch: 'main' },
		],
	});
	paintTabStrip(grid, 0, side + 1, [
		{ title: 'build', state: claudeState, active: true },
		{ title: 'docs', state: 'done', unread: true },
		{ title: 'shell' },
	]);
	paintPanel(grid, {
		top: 1,
		left: 33,
		bottom: height - 1,
		right: 89,
		title: 'claude-1',
		state: claudeState,
		focused: true,
	});
	paintPanel(grid, {
		top: 1,
		left: 90,
		bottom: 17,
		right: width - 1,
		title: 'reviewer',
		state: reviewer,
		unread: phase >= 2,
		focused: false,
	});
	paintPanel(grid, {
		top: 18,
		left: 90,
		bottom: height - 1,
		right: width - 1,
		title: 'pwsh',
		focused: false,
	});

	const left = 35;
	grid.run(3, left, [
		['> ', 'focus'],
		['Add the install section with OS tabs', 'primary', { bold: true }],
	]);
	step(grid, 5, left, 'Read ', 'apps/website/src/Website.tsx');
	step(grid, 6, left, 'Edit ', 'apps/website/src/Website.tsx');
	grid.run(6, 82, [
		['+64', 'success'],
		[' -3', 'error'],
	]);
	if (phase === 0) working(grid, 8, left, '0m 12s · Esc to interrupt');
	if (phase >= 1) {
		step(grid, 7, left, 'Edit ', 'locales/en-US/website.ts');
		grid.put(7, 82, '+18', 'success');
		step(grid, 8, left, 'Edit ', 'locales/zh-CN/website.ts');
		grid.put(8, 82, '+18', 'success');
		step(grid, 9, left, 'Run  ', 'pnpm --filter @orchbee/website test');
	}
	if (phase === 1) working(grid, 11, left, '0m 48s · Esc to interrupt');
	if (phase >= 2) {
		grid.run(10, left + 2, [
			['14 passed', 'success'],
			['  1.2s', 'secondary'],
		]);
		grid.put(12, left, 'The install section is in. Each OS gets one command');
		grid.put(13, left, 'and a copy button. Pushing the branch next.');
	}
	if (phase === 2) working(grid, 15, left, '1m 21s · Esc to interrupt');
	if (phase === 3) {
		grid.frame(15, left, 21, 87, 'warning', 'round');
		grid.put(15, left + 2, ' Run this command? ', 'warning', { bold: true });
		grid.put(17, left + 3, 'git push origin feat/website', 'primary', { bold: true });
		grid.run(19, left + 3, [
			['› ', 'focus'],
			['Yes', 'primary', { bold: true }],
			['     Yes, and don’t ask again', 'secondary'],
			['     No', 'secondary'],
		]);
		grid.run(23, left, [
			['Esc', 'primary', { bold: true }],
			[' cancel   ', 'secondary'],
			['Enter', 'primary', { bold: true }],
			[' confirm', 'secondary'],
		]);
	}

	const review = 92;
	grid.run(3, review, [
		['> ', 'focus'],
		['Review feat/website. Focus on a11y and zh-CN copy.', 'primary', { bold: true }],
	]);
	grid.run(5, review, [
		['● ', 'secondary'],
		['git diff main...feat/website --stat', 'secondary'],
	]);
	grid.put(6, review + 2, '3 files changed, 100 insertions(+), 3 deletions(-)', 'secondary');
	if (phase === 0) working(grid, 8, review, '1m 02s · Esc to interrupt');
	if (phase >= 1) {
		step(grid, 7, review, 'Checked ', 'tab focus order');
		step(grid, 8, review, 'Checked ', 'zh-CN line breaks at 320px');
	}
	if (phase === 1) working(grid, 10, review, '1m 37s · Esc to interrupt');
	if (phase >= 2) {
		grid.put(10, review, 'No blocking issues. Two notes:');
		grid.put(11, review, '1. The copy button is skipped by Tab on macOS.', 'secondary');
		grid.put(12, review, '2. The zh-CN heading wraps badly at 320px.', 'secondary');
	}

	grid.run(20, review, [['PS> ', 'secondary'], ['pnpm --filter @orchbee/website dev']]);
	grid.put(22, review, 'Rsbuild v2.1.6', 'primary', { bold: true });
	grid.run(24, review, [
		['  Local:    ', 'secondary'],
		['http://127.0.0.1:3100/', 'info'],
	]);
	const builds = ['0.33s', '0.04s', '0.05s'].slice(0, Math.min(phase, 2) + 1);
	builds.forEach((time, index) => {
		grid.run(26 + index * 2, review, [
			['start   ', 'secondary'],
			['build started...', 'secondary'],
		]);
		grid.run(27 + index * 2, review, [['ready   ', 'success'], [`built in ${time}`]]);
	});
	return grid.lines();
}

export function compactHeroFrame(phase: HeroPhase): TuiLine[] {
	const width = COMPACT_HERO_COLUMNS;
	const height = 25;
	const grid = createGrid(width, height);
	const claude: AgentState = phase < 3 ? 'working' : 'attention';
	paintTabStrip(grid, 0, 0, [
		{ title: 'build', state: claude, active: true },
		{ title: 'docs', state: 'done', unread: true },
		{ title: 'shell' },
	]);
	paintPanel(grid, {
		top: 1,
		left: 0,
		bottom: height - 1,
		right: width - 1,
		title: 'claude-1',
		state: claude,
		focused: true,
	});
	const left = 2;
	grid.run(3, left, [
		['> ', 'focus'],
		['Add the install section with OS tabs', 'primary', { bold: true }],
	]);
	step(grid, 5, left, 'Edit ', 'Website.tsx');
	grid.run(5, 45, [
		['+64', 'success'],
		[' -3', 'error'],
	]);
	if (phase === 0) working(grid, 7, left, '0m 12s');
	if (phase >= 1) {
		step(grid, 6, left, 'Edit ', 'en-US/website.ts');
		grid.put(6, 45, '+18', 'success');
		step(grid, 7, left, 'Edit ', 'zh-CN/website.ts');
		grid.put(7, 45, '+18', 'success');
		step(grid, 8, left, 'Run  ', 'pnpm test');
	}
	if (phase === 1) working(grid, 10, left, '0m 48s');
	if (phase >= 2) {
		grid.run(9, left + 2, [
			['14 passed', 'success'],
			['  1.2s', 'secondary'],
		]);
		grid.put(11, left, 'The install section is in. Each OS gets');
		grid.put(12, left, 'one command and a copy button.');
	}
	if (phase === 2) working(grid, 14, left, '1m 21s');
	if (phase === 3) {
		grid.frame(14, left, 19, width - 3, 'warning', 'round');
		grid.put(14, left + 2, ' Run this command? ', 'warning', { bold: true });
		grid.put(16, left + 3, 'git push origin feat/website', 'primary', { bold: true });
		grid.run(17, left + 3, [
			['› ', 'focus'],
			['Yes', 'primary', { bold: true }],
			['    Always', 'secondary'],
			['    No', 'secondary'],
		]);
		grid.run(21, left, [
			['Esc', 'primary', { bold: true }],
			[' cancel   ', 'secondary'],
			['Enter', 'primary', { bold: true }],
			[' confirm', 'secondary'],
		]);
	}
	return grid.lines();
}
