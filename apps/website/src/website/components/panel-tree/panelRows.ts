import { green, red, row, textRow, type TerminalRow } from '../terminal-text/terminal-row';

// What the Panels of a try-it window show: every Panel is a PowerShell prompt, the first one after
// a `git status -sb`. Only the focused Panel shows the cursor. English in every page language.

const PROMPT = 'PS C:\\demo\\shop-api> ';
const FIRST_PANEL = 1;
const STATUS: readonly TerminalRow[] = [
	textRow(`${PROMPT}git status -sb`),
	row`## ${green('main')}...${red('origin/main')} [ahead ${green('1')}]`,
	row` ${red('M')} src/orders/routes.ts`,
];
const PROMPT_ROW = textRow(PROMPT);
const FOCUSED_PROMPT_ROW = textRow(PROMPT, { cursor: 'blink' });

/** The rows of Panel `id`, with the cursor when it has focus. */
export function panelRows(id: number, focused: boolean): readonly TerminalRow[] {
	const prompt = focused ? FOCUSED_PROMPT_ROW : PROMPT_ROW;
	return id === FIRST_PANEL ? [...STATUS, prompt] : [prompt];
}
