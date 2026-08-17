import * as React from 'react';

import styles from './FieldControl.module.css';

interface FieldControlProps {
	readonly children: React.ReactNode;
	readonly error: string | null;
}

export function FieldControl({ children, error }: FieldControlProps): React.ReactNode {
	return (
		<div className={styles['root']}>
			{children}
			{error === null ? null : (
				<span className={styles['error']} role='alert'>
					{error}
				</span>
			)}
		</div>
	);
}
