import type { ReactElement, ReactNode } from 'react';

import styles from './DesktopWindow.module.css';

/**
 * The sizes a drawn desktop application comes in: `full` narrows to the mini and phone layouts;
 * `mini` starts at the mini layout and narrows to the phone layout.
 */
export type DesktopWindowSize = 'full' | 'mini';

/**
 * The Orchbee desktop application in its window: the optional sidebar beside the main area
 * (`children`: a Tab strip and the Panels). Its colors are the product's own system colors, and its
 * root scopes the product's appearance: it is dark, and `data-theme='light'` set on it gives the
 * product's light look. Place it in a WindowFit. It is a picture, labelled by `label`.
 */
export function DesktopWindow({
	label,
	size,
	sidebar,
	children,
}: {
	label: string;
	size: DesktopWindowSize;
	sidebar?: ReactNode;
	children: ReactNode;
}): ReactElement {
	return (
		<div
			className={sidebar === undefined ? styles['window'] : styles['windowWithSidebar']}
			data-size={size}
			data-theme='dark'
			role='img'
			aria-label={label}
		>
			{sidebar === undefined ? null : <div className={styles['sidebar']}>{sidebar}</div>}
			<div className={styles['main']}>{children}</div>
		</div>
	);
}
