import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
import styles from './WindowFrame.module.css';

// A desktop window around a picture: the title bar with its three lights, then the content.
// A `closeLabel` turns the red light into the close button the hero story listens to.
export function WindowFrame({
	title,
	size = 'regular',
	closeLabel,
	focusRing = false,
	barEnd,
	children,
}: {
	title?: ReactNode;
	size?: 'regular' | 'small';
	closeLabel?: string;
	focusRing?: boolean;
	barEnd?: ReactNode;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={joinClassNames(styles['window'], styles[size], focusRing && styles['ring'])}>
			<div className={styles['bar']}>
				<span
					className={joinClassNames(styles['lights'], closeLabel !== undefined && styles['live'])}
				>
					{closeLabel === undefined ? (
						<i />
					) : (
						<button
							className={styles['close']}
							type='button'
							data-action='close-window'
							aria-label={closeLabel}
							title={closeLabel}
						>
							<svg viewBox='0 0 10 10' aria-hidden='true'>
								<path
									d='M2.5 2.5l5 5M7.5 2.5l-5 5'
									fill='none'
									stroke='currentColor'
									strokeWidth='1.6'
									strokeLinecap='round'
								/>
							</svg>
						</button>
					)}
					<i />
					<i />
				</span>
				{title === undefined ? null : <span className={styles['title']}>{title}</span>}
				{barEnd}
			</div>
			{children}
		</div>
	);
}
