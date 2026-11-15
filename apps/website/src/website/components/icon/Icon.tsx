import type { ReactElement } from 'react';

import styles from './Icon.module.css';

// Line icons drawn on a 16-unit grid in the current text color.
const PATHS = {
	copy: (
		<>
			<rect x='5.5' y='5.5' width='8' height='8' rx='1.7' />
			<path d='M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2' />
		</>
	),
	download: <path d='M8 2v8m0 0L4.8 6.8M8 10l3.2-3.2M3 13.2h10' />,
} as const;

export type IconName = keyof typeof PATHS;

/** A decorative icon; the control around it carries the accessible name. */
export function Icon({ name }: { name: IconName }): ReactElement {
	return (
		<svg
			className={styles['icon']}
			viewBox='0 0 16 16'
			fill='none'
			stroke='currentColor'
			strokeWidth='1.6'
			strokeLinecap='round'
			strokeLinejoin='round'
			aria-hidden='true'
		>
			{PATHS[name]}
		</svg>
	);
}
