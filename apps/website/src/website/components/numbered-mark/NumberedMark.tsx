import type { ReactElement } from 'react';

import styles from './NumberedMark.module.css';

/**
 * A numbered disc on a figure whose parts are explained by notes beside it. The figure marks its
 * scope with `data-marks`; inside it, an element with `data-mark='<number>'` is a part of the
 * drawing, and one that also carries `data-mark-note` is the note for that number, which takes
 * keyboard focus once script runs. Everything naming the same number lights up together, carrying
 * `data-hot`, while one of them is under the pointer or its note has focus.
 *
 * The disc fills while it, or an element around it, is lit. Its number is a visual cue; the notes
 * carry the meaning, so assistive technology skips it. To place it on a drawing, wrap it in an
 * element of the figure's own styles.
 */
export function NumberedMark({ number }: { number: number }): ReactElement {
	return (
		<span className={styles['mark']} aria-hidden='true'>
			{number}
		</span>
	);
}
