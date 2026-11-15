import type { ReactElement, ReactNode } from 'react';

import styles from './NumberedMarks.module.css';

/**
 * A figure whose parts carry numbered marks explained by notes beside it. Inside it, an element
 * with `data-mark` names its mark's number: a part of the drawing, or, also marked
 * `data-mark-note`, the note that explains it, which takes keyboard focus once script runs.
 * Everything naming the same number lights up together, carrying `data-hot`, while one of them is
 * under the pointer or its note has focus; style the lit state by that attribute. The scope adds
 * no box of its own.
 */
export function MarkScope({ children }: { children: ReactNode }): ReactElement {
	return (
		<div className={styles['scope']} data-marks=''>
			{children}
		</div>
	);
}

/**
 * The numbered disc itself. It fills while it, or the element it sits in, is lit. The number is a
 * visual cue; the notes carry the meaning, so assistive technology skips it. To place the disc in
 * a drawing, wrap it in an element of the figure's own styles.
 */
export function NumberedMark({ number }: { number: number }): ReactElement {
	return (
		<span className={styles['mark']} aria-hidden='true'>
			{number}
		</span>
	);
}
