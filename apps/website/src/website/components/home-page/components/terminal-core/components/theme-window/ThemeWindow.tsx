import { useOrchbeeIntl } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { DEFAULT_TERMINAL_THEME } from '../../terminal-themes';
import styles from './ThemeWindow.module.css';

/** The ANSI colors the sample names, by palette index. */
type Ansi = 1 | 2 | 3 | 6 | 8;

/** A run of sample text: plain, in an ANSI color, bold, or selected. */
type Segment =
	| string
	| { readonly text: string; readonly ansi?: Ansi; readonly bold?: true; readonly selected?: true };

const PROMPT = 'PS C:\\demo\\shop-api> ';

// A short session in shop-api, English in every language, that uses the colors a theme changes
// most visibly. The last row ends at the cursor.
const ROWS: readonly (readonly Segment[])[] = [
	[`${PROMPT}git log --oneline -2`],
	[
		{ text: '2905180', selected: true },
		' (',
		{ text: 'HEAD ->', ansi: 6, bold: true },
		' ',
		{ text: 'main', ansi: 2, bold: true },
		', ',
		{ text: 'origin/main', ansi: 1, bold: true },
		') Add order routes',
	],
	[{ text: 'a6baeab', ansi: 3 }, ' Initial service skeleton'],
	[`${PROMPT}git diff --stat`],
	[' src/orders/routes.ts   | 14 ', { text: '+++++++++', ansi: 2 }, { text: '-----', ansi: 1 }],
	[' src/orders/validate.ts |  9 ', { text: '++++++', ansi: 2 }, { text: '---', ansi: 1 }],
	[
		' test/orders.test.ts    | 27 ',
		{ text: '++++++++++++++++++++++++++', ansi: 2 },
		{ text: '-', ansi: 1 },
	],
	[' 3 files changed, 41 insertions(+), 9 deletions(-)'],
	[`${PROMPT}npm test -- orders`],
	[],
	[
		' ',
		{ text: '✓', ansi: 2 },
		' test/orders.test.ts ',
		{ text: '(12 tests)', ansi: 8 },
		' ',
		{ text: '41ms', ansi: 3 },
	],
	[],
	[
		{ text: ' Test Files ', ansi: 8 },
		' ',
		{ text: '1 passed', ansi: 2, bold: true },
		' ',
		{ text: '(1)', ansi: 8 },
	],
	[
		{ text: '      Tests ', ansi: 8 },
		' ',
		{ text: '12 passed', ansi: 2, bold: true },
		' ',
		{ text: '(12)', ansi: 8 },
	],
	[{ text: '   Duration ', ansi: 8 }, ' 612ms'],
	[PROMPT],
];

function SegmentText({ segment }: { segment: Segment }): ReactElement | string {
	if (typeof segment === 'string') return segment;
	if (segment.selected === true) return <span className={styles['selected']}>{segment.text}</span>;
	if (segment.bold === true) return <b data-ansi={segment.ansi}>{segment.text}</b>;
	return <span data-ansi={segment.ansi}>{segment.text}</span>;
}

// A row's identity is its line in the sample and a segment's its place in the row; neither moves.
function byPosition<Item>(items: readonly Item[]): { key: string; item: Item }[] {
	return items.map((item, position) => ({ key: String(position), item }));
}

const SWATCHES = Array.from({ length: 16 }, (_, index) => index);

/**
 * A window that shows one terminal theme: its name in the title bar, a sample of terminal output,
 * and its sixteen ANSI colors in a strip. Every color is a --ob-comp-website-terminal-* property of
 * the window, so applying a theme to the window (`data-theme-window`) recolors all of it; the
 * title (`data-theme-name`) is written with it. It shows the default theme as prerendered.
 */
export function ThemeWindow(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['window']} data-theme-window=''>
			<div className={styles['bar']}>
				<b className={styles['name']} data-theme-name=''>
					{t(DEFAULT_TERMINAL_THEME.name)}
				</b>
			</div>
			<div className={styles['sample']}>
				{byPosition(ROWS).map(({ key, item: segments }, row) => (
					<div key={key} className={styles['row']}>
						{byPosition(segments).map(({ key: at, item: segment }) => (
							<SegmentText key={at} segment={segment} />
						))}
						{row === ROWS.length - 1 ? <span className={styles['cursor']} /> : null}
					</div>
				))}
			</div>
			<div className={styles['swatches']} aria-hidden='true'>
				{SWATCHES.map((index) => (
					<span key={index} data-index={index} />
				))}
			</div>
		</div>
	);
}
