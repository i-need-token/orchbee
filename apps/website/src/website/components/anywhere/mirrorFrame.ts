// The claude-1 Panel every attached client shows. Its prompt rows are page elements laid over the
// frame, so a line typed into any client appears in all of them.
import { createGrid, paintPanel, paintTabStrip, type TuiLine } from '../../../tui-grid';

const MIRROR_COLUMNS = 44;

export function mirrorFrame(state: 'working' | 'done'): TuiLine[] {
	const width = MIRROR_COLUMNS;
	const height = 15;
	const grid = createGrid(width, height);
	paintTabStrip(grid, 0, 0, [
		{ title: 'claude-1', state, active: true },
		{ title: 'docs', state: 'done' },
		{ title: 'shell' },
	]);
	paintPanel(grid, {
		top: 1,
		left: 0,
		bottom: height - 1,
		right: width - 1,
		title: 'claude-1',
		state,
		focused: true,
	});
	grid.run(3, 2, [
		['> ', 'secondary'],
		['Add the install section', 'secondary'],
	]);
	grid.run(4, 2, [['● ', 'secondary'], ['Edit '], ['Website.tsx', 'secondary']]);
	grid.run(4, 36, [
		['+64', 'success'],
		[' -3', 'error'],
	]);
	grid.run(5, 2, [['● ', 'secondary'], ['Run  '], ['pnpm test', 'secondary']]);
	grid.run(6, 4, [
		['14 passed', 'success'],
		['  1.2s', 'secondary'],
	]);
	grid.put(8, 2, 'The install section is in.');
	return grid.lines();
}
