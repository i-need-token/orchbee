import { DialogDescription, DialogHeader, DialogTitle } from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './ResourceDialogHeader.module.css';

interface ResourceDialogHeaderProps {
	readonly description: string;
	readonly title: string;
}

export function ResourceDialogHeader({
	description,
	title,
}: ResourceDialogHeaderProps): React.ReactNode {
	return (
		<DialogHeader className={styles['header']}>
			<DialogTitle className={styles['title']}>{title}</DialogTitle>
			<DialogDescription className={styles['description']}>{description}</DialogDescription>
		</DialogHeader>
	);
}
