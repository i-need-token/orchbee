import type { ReactElement } from 'react';

import styles from './Icon.module.css';

// Line icons drawn on a 16-unit grid in the current text color. The platform marks are filled instead:
// `apple` is the "apple" path of Simple Icons (CC0 1.0, https://github.com/simple-icons/simple-icons,
// version 16.38.0), scaled and centered on the same grid; `windows` is a four-pane mark drawn on it.
const PATHS = {
	apple: (
		<path
			fill='currentColor'
			stroke='none'
			transform='translate(1.76 1.36) scale(0.52)'
			d='M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701'
		/>
	),
	close: <path d='M3.5 3.5l9 9m0-9l-9 9' />,
	copy: (
		<>
			<rect x='5.5' y='5.5' width='8' height='8' rx='1.7' />
			<path d='M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2' />
		</>
	),
	download: <path d='M8 2v8m0 0L4.8 6.8M8 10l3.2-3.2M3 13.2h10' />,
	windows: (
		<path
			fill='currentColor'
			stroke='none'
			d='M2 1.6h5.3v5.3H2zm6.7 0h5.3v5.3H8.7zm-6.7 6.7h5.3v5.3H2zm6.7 0h5.3v5.3H8.7z'
		/>
	),
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
