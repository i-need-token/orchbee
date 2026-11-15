import type { ReactElement } from 'react';

import { joinClassNames } from '../../joinClassNames';
import styles from './WindowCaption.module.css';

/**
 * The minimize, maximize and close glyphs of a Windows title bar, drawn, not operable. `overlay`
 * pins them over the end of a centered title; `end` closes a row of tabs. `narrow` is the host
 * terminal's tighter spacing.
 */
export function WindowCaption({
	placement,
	spacing = 'regular',
}: {
	placement: 'overlay' | 'end';
	spacing?: 'regular' | 'narrow';
}): ReactElement {
	return (
		<span
			className={joinClassNames(styles['caption'], styles[placement], styles[spacing])}
			aria-hidden='true'
		>
			<i>
				<svg viewBox='0 0 10 10'>
					<path d='M1 5.5h8' />
				</svg>
			</i>
			<i>
				<svg viewBox='0 0 10 10'>
					<rect x='1.5' y='1.5' width='7' height='7' />
				</svg>
			</i>
			<i>
				<svg viewBox='0 0 10 10'>
					<path d='M1.5 1.5l7 7m0-7l-7 7' />
				</svg>
			</i>
		</span>
	);
}
