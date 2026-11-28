import type * as React from 'react';

import type { VirtualTreeHighlight } from '../../model/virtual-tree-types';
import styles from './VirtualTreeLabel.module.css';

export interface VirtualTreeLabelProps {
	readonly text: string;
	/** The characters to highlight, in order and apart, such as a row state's `highlights`. */
	readonly highlights: readonly VirtualTreeHighlight[];
}

/**
 * A row's text with the characters the find matched highlighted, as VS Code's highlighted label
 * draws them. It adds no element of its own around the text, so the element holding it keeps
 * clipping it; that element lays the parts out as one line of text, as a name's element does, where
 * the row's flex content would make each part an item of its own.
 */
export function VirtualTreeLabel({ highlights, text }: VirtualTreeLabelProps): React.ReactNode {
	if (highlights.length === 0) return text;
	const parts: React.ReactNode[] = [];
	let position = 0;
	for (const { end, start } of highlights) {
		if (end <= start || end <= position) continue;
		if (position < start) parts.push(text.slice(position, start));
		const from = Math.max(position, start);
		parts.push(
			<span className={styles['highlight']} key={from}>
				{text.slice(from, end)}
			</span>,
		);
		position = end;
	}
	if (position < text.length) parts.push(text.slice(position));
	return parts;
}
