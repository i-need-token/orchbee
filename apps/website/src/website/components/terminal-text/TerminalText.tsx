import type { ReactElement } from 'react';

import { joinClassNames } from '../../joinClassNames';
import styles from './TerminalText.module.css';
import type { TerminalRow, TerminalSegment } from './terminal-row';

/** Who draws the terminal: a Panel of Orchbee, or the plain console of another terminal. */
export type TerminalHost = 'panel' | 'console';

function Segment({ segment }: { segment: TerminalSegment }): ReactElement | string {
	if (typeof segment === 'string') return segment;
	if ('cursor' in segment)
		return (
			<i className={styles['cursor']} data-blink={segment.cursor === 'blink' ? '' : undefined} />
		);
	if (segment.tone === 'bold') return <b>{segment.text}</b>;
	return <span data-tone={segment.tone}>{segment.text}</span>;
}

/**
 * The text area of a drawn terminal. It shows whole rows only, the last ones when the text is
 * taller than the area, in the window's terminal palette. `bind` names the scene rows that script
 * writes here; the `<template>` holds the elements it clones to write them.
 */
// A row's identity is its line on the screen, and a segment's its place in the row: rows are
// replaced, never moved.
function byPosition<Item>(items: readonly Item[]): { key: string; item: Item }[] {
	return items.map((item, position) => ({ key: String(position), item }));
}

export function TerminalText({
	rows,
	host,
	bind,
}: {
	rows: readonly TerminalRow[];
	host: TerminalHost;
	bind?: string | undefined;
}): ReactElement {
	return (
		<div className={joinClassNames(styles['area'], styles[host])} data-terminal=''>
			<div className={styles['view']}>
				<div className={styles['rows']} data-scene-rows={bind}>
					{byPosition(rows).map(({ key, item: row }) => (
						<div key={key} className={styles['row']} data-input={row.input ? '' : undefined}>
							{byPosition(row.segments).map(({ key: at, item: segment }) => (
								<Segment key={at} segment={segment} />
							))}
						</div>
					))}
				</div>
			</div>
			{bind === undefined ? null : (
				<template>
					<div className={styles['row']} data-part='row' />
					<i className={styles['cursor']} data-part='cursor' />
				</template>
			)}
		</div>
	);
}
