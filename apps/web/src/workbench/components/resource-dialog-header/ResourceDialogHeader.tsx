import { DialogDescription, DialogHeader, DialogTitle } from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './ResourceDialogHeader.module.css';

interface ResourceDialogHeaderProps {
	/** Absent when the title alone says what the form changes. */
	readonly description?: string | undefined;
	readonly title: string;
	/** Lets a Dialog's only field take the title as its label. */
	readonly titleId?: string | undefined;
}

export function ResourceDialogHeader({
	description,
	title,
	titleId,
}: ResourceDialogHeaderProps): React.ReactNode {
	return (
		<DialogHeader className={styles['header']}>
			<DialogTitle className={styles['title']} id={titleId}>
				{title}
			</DialogTitle>
			{description === undefined ? null : (
				<DialogDescription className={styles['description']}>{description}</DialogDescription>
			)}
		</DialogHeader>
	);
}
