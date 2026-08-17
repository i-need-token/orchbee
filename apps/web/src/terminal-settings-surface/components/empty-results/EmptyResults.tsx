import * as React from 'react';

import styles from './EmptyResults.module.css';

interface EmptyResultsProps {
	readonly message: string;
}

export function EmptyResults({ message }: EmptyResultsProps): React.ReactNode {
	return <div className={styles['root']}>{message}</div>;
}
