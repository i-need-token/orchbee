import type { ReactElement, ReactNode } from 'react';

import { WindowCaption } from '../window-caption/WindowCaption';
import styles from './TuiWindow.module.css';

/**
 * The sizes a drawn terminal interface comes in: `full` narrows to the mini and phone layouts;
 * `play`, the window of a PanelPlay, narrows to its phone layout.
 */
export type TuiWindowSize = 'full' | 'play';

/**
 * The Orchbee terminal interface in the window of the host terminal that runs it: the title bar,
 * then the optional sidebar beside the main area (`children`: a Tab strip and the Panels). Place
 * it in a WindowFit. It is a picture, labelled by `label`.
 */
export function TuiWindow({
	label,
	size,
	sidebar,
	children,
}: {
	label: string;
	size: TuiWindowSize;
	sidebar?: ReactNode;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={styles['window']} data-size={size} role='img' aria-label={label}>
			<div className={styles['bar']}>
				<span>orchbee</span>
				<WindowCaption placement='overlay' spacing='narrow' />
			</div>
			<div className={sidebar === undefined ? styles['screen'] : styles['screenWithSidebar']}>
				{sidebar === undefined ? null : <div className={styles['sidebar']}>{sidebar}</div>}
				<div className={styles['main']}>{children}</div>
			</div>
		</div>
	);
}
