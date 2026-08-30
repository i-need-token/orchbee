import * as React from 'react';

import styles from './FieldControl.module.css';

interface FieldControlProps {
	readonly children: React.ReactNode;
	readonly error: string | null;
	readonly errorId?: string | undefined;
}

export function FieldControl({ children, error, errorId }: FieldControlProps): React.ReactNode {
	return (
		<div className={styles['root']}>
			{children}
			{error === null ? null : (
				<span className={styles['error']} id={errorId} role='alert'>
					{error}
				</span>
			)}
		</div>
	);
}
