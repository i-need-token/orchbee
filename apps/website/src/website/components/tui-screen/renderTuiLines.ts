import type { TuiLine } from '../../../tui-grid';
import styles from './TuiScreen.module.css';

// Repaints a TuiScreen in the browser with the same elements and classes the static page has.
export function renderTuiLines(screen: Element, lines: readonly TuiLine[]): void {
	const target = screen.querySelector('[data-tui-lines]');
	if (target === null) return;
	const document = screen.ownerDocument;
	const rows = lines.map((line) => {
		const row = document.createElement('div');
		row.className = styles['line'] ?? '';
		for (const segment of line) {
			const cell = document.createElement(segment.bold ? 'b' : 'span');
			cell.className = [
				styles[`tone-${segment.tone}`],
				styles[`surface-${segment.surface}`],
				segment.shape === undefined ? undefined : styles['shape'],
				segment.shape === undefined ? undefined : styles[`shape-${segment.shape}`],
				segment.motion === undefined ? undefined : styles[`motion-${segment.motion}`],
			]
				.filter((name) => name !== undefined && name !== '')
				.join(' ');
			cell.textContent = segment.text;
			row.append(cell);
		}
		return row;
	});
	target.replaceChildren(...rows);
}
