import * as React from 'react';

import styles from './ShellSetupCard.module.css';

interface ShellSetupCardProps {
	readonly command: string;
	readonly commandLabel: string;
	readonly description?: string | undefined;
	readonly title: string;
}

export function ShellSetupCard({
	command,
	commandLabel,
	description,
	title,
}: ShellSetupCardProps): React.ReactNode {
	return (
		<section className={styles['card']}>
			<h2 className={styles['title']}>{title}</h2>
			{description === undefined ? null : <p className={styles['description']}>{description}</p>}
			<span className={styles['label']}>{commandLabel}</span>
			<code className={styles['command']}>{command}</code>
		</section>
	);
}
