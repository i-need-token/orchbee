import type { ReactElement } from 'react';

import styles from './IconSprite.module.css';

// The symbols every picture on the page draws with <use>, declared once per page.
export function IconSprite(): ReactElement {
	return (
		<svg className={styles['sprite']} width='0' height='0' aria-hidden='true'>
			<symbol id='i-dl' viewBox='0 0 24 24'>
				<path
					d='M12 4v11M7 10l5 5 5-5M5 20h14'
					fill='none'
					stroke='currentColor'
					strokeWidth='2'
					strokeLinecap='round'
					strokeLinejoin='round'
				/>
			</symbol>
			<symbol id='i-gh' viewBox='0 0 24 24'>
				<path
					fill='currentColor'
					d='M12 .3a12 12 0 0 0-3.8 23.4c.6.1.8-.3.8-.6v-2c-3.3.7-4-1.6-4-1.6-.6-1.4-1.4-1.8-1.4-1.8-1-.7.1-.7.1-.7 1.2 0 1.9 1.2 1.9 1.2 1 1.8 2.8 1.3 3.5 1 0-.8.4-1.3.7-1.6-2.7-.3-5.5-1.3-5.5-6 0-1.2.5-2.3 1.3-3.1-.2-.4-.6-1.6 0-3.2 0 0 1-.3 3.4 1.2a11.5 11.5 0 0 1 6 0c2.3-1.5 3.3-1.2 3.3-1.2.6 1.6.2 2.8 0 3.2.9.8 1.3 1.9 1.3 3.2 0 4.6-2.8 5.6-5.5 5.9.5.4.9 1 .9 2.2v3.3c0 .3.1.7.8.6A12 12 0 0 0 12 .3'
				/>
			</symbol>
			<symbol id='i-copy' viewBox='0 0 24 24'>
				<g
					fill='none'
					stroke='currentColor'
					strokeWidth='1.8'
					strokeLinecap='round'
					strokeLinejoin='round'
				>
					<rect x='9' y='9' width='11' height='11' rx='2' />
					<path d='M5 15V5a2 2 0 0 1 2-2h10' />
				</g>
			</symbol>
			<symbol id='i-ok' viewBox='0 0 24 24'>
				<path
					d='m5 12 5 5 9-10'
					fill='none'
					stroke='currentColor'
					strokeWidth='2'
					strokeLinecap='round'
					strokeLinejoin='round'
				/>
			</symbol>
			<symbol id='i-win' viewBox='0 0 24 24'>
				<g
					fill='none'
					stroke='currentColor'
					strokeWidth='1.8'
					strokeLinecap='round'
					strokeLinejoin='round'
				>
					<rect x='3' y='4' width='18' height='16' rx='2' />
					<path d='M3 9h18' />
				</g>
			</symbol>
			<symbol id='i-replay' viewBox='0 0 24 24'>
				<g
					fill='none'
					stroke='currentColor'
					strokeWidth='2'
					strokeLinecap='round'
					strokeLinejoin='round'
				>
					<path d='M3 12a9 9 0 1 0 2.6-6.4L3 8' />
					<path d='M3 3v5h5' />
				</g>
			</symbol>
			<symbol id='i-pause' viewBox='0 0 24 24'>
				<path
					d='M8 5v14M16 5v14'
					fill='none'
					stroke='currentColor'
					strokeWidth='2.4'
					strokeLinecap='round'
				/>
			</symbol>
			<symbol id='i-play' viewBox='0 0 24 24'>
				<path
					d='M8 5.5v13a1 1 0 0 0 1.5.86l10.4-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5Z'
					fill='currentColor'
				/>
			</symbol>
		</svg>
	);
}
