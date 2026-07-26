import styles from './GroupEditorMenuIcon.module.css';
import { type ReactNode } from 'react';

export function GroupEditorMenuIcon({
	kind,
}: {
	kind: 'new-tab' | 'move-window' | 'close' | 'ungroup';
}): ReactNode {
	const paths: Record<typeof kind, ReactNode> = {
		'new-tab': (
			<>
				<rect x='4' y='4' width='14' height='14' rx='1' />
				<path d='M11 7v8M7 11h8' />
			</>
		),
		'move-window': (
			<>
				<rect x='4' y='6' width='14' height='13' rx='1' />
				<path d='M10 3h10v10m0-10-8 8' />
			</>
		),
		close: (
			<>
				<rect x='4' y='4' width='14' height='14' rx='1' />
				<path d='m8 8 6 6m0-6-6 6' />
			</>
		),
		ungroup: (
			<>
				<path d='M8 7H5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2v-3' />
				<path d='M10 10 20 4m-5 0h5v5' />
			</>
		),
	};
	return (
		<svg className={styles['ct-group-editor__menu-icon']} viewBox='0 0 24 24' aria-hidden='true'>
			{paths[kind]}
		</svg>
	);
}
