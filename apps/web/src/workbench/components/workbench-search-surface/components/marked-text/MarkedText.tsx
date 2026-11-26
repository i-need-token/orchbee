import * as React from 'react';

import type { WorkbenchSearchTextRange } from '../../../../types/workbench-search-provider';

import styles from './MarkedText.module.css';

interface MarkedTextProps {
	readonly text: string;
	/** What to mark, ascending and apart; spans past the text are cut to it. */
	readonly ranges?: readonly WorkbenchSearchTextRange[] | undefined;
}

/** A search row's text with what the query matched marked. */
export function MarkedText({ text, ranges }: MarkedTextProps): React.ReactNode {
	if (ranges === undefined || ranges.length === 0) return text;
	const parts: React.ReactNode[] = [];
	let at = 0;
	for (const range of ranges) {
		const start = Math.max(at, Math.min(range.start, text.length));
		const end = Math.max(start, Math.min(range.end, text.length));
		if (start === end) continue;
		if (start > at) parts.push(text.slice(at, start));
		parts.push(
			<mark className={styles['match']} key={start}>
				{text.slice(start, end)}
			</mark>,
		);
		at = end;
	}
	if (at < text.length) parts.push(text.slice(at));
	return parts;
}
