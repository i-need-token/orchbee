import type { ReactElement, ReactNode } from 'react';

import { FigureCaption } from '../figure-caption/FigureCaption';
import { WindowFit } from '../window-fit/WindowFit';
import styles from './PageStage.module.css';

/**
 * The drawn window under a client page's head (`children`, placed in a WindowFit), rising into
 * place as the page opens, and the caption under it with an optional `aside` such as a legend.
 */
export function PageStage({
	caption,
	aside,
	children,
}: {
	caption: ReactNode;
	aside?: ReactNode;
	children: ReactNode;
}): ReactElement {
	return (
		<>
			<div className={styles['stage']}>
				<WindowFit>{children}</WindowFit>
			</div>
			<FigureCaption aside={aside}>{caption}</FigureCaption>
		</>
	);
}
