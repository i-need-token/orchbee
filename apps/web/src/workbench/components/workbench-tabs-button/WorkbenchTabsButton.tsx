import type { ReactNode } from 'react';

import styles from './WorkbenchTabsButton.module.css';

interface WorkbenchTabsButtonProps {
	readonly count: number;
	/** The accessible name, which says how many Tabs are open. */
	readonly label: string;
	/** Absent, the button is the hidden copy that keeps the bar's middle centered. */
	readonly onToggle?: (() => void) | undefined;
	readonly open: boolean;
	readonly touch: boolean;
	readonly word: string;
}

/** `Tabs 8`: the word, then the count in the secondary color; it opens and closes the Tab list. */
export function WorkbenchTabsButton({
	count,
	label,
	onToggle,
	open,
	touch,
	word,
}: WorkbenchTabsButtonProps): ReactNode {
	const face = (
		<span className={styles['face']}>
			<span className={styles['word']}>{word}</span>
			<span className={styles['count']}>{count}</span>
		</span>
	);
	if (onToggle === undefined) {
		return (
			<span
				aria-hidden='true'
				className={styles['root']}
				data-ghost=''
				data-touch={touch || undefined}
			>
				{face}
			</span>
		);
	}
	return (
		<button
			aria-label={label}
			aria-pressed={open}
			className={styles['root']}
			data-open={open || undefined}
			data-slot='workbench-tabs-button'
			data-touch={touch || undefined}
			type='button'
			onClick={(event) => {
				// The bar's own click opens the Tab list; this button keeps its click to itself.
				event.stopPropagation();
				onToggle();
			}}
		>
			{face}
		</button>
	);
}
