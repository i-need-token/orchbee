import type { ReactElement, ReactNode } from 'react';

import styles from './SessionScene.module.css';
import { useSessionStrings } from './useSessionStrings';

/**
 * Binds what it contains to the scripted Session: activation replays the Session in every element
 * inside that names a Session binding key, from the status line to the drawn windows. It carries
 * the page-language words the Session shows, for script to read. It draws no box of its own.
 */
export function SessionScene({ children }: { children: ReactNode }): ReactElement {
	const { entries } = useSessionStrings();
	return (
		<div className={styles['scene']} data-scene='session'>
			<template data-scene-strings=''>
				{entries.map(([key, text]) => (
					<span key={key} data-key={key}>
						{text}
					</span>
				))}
			</template>
			{children}
		</div>
	);
}
