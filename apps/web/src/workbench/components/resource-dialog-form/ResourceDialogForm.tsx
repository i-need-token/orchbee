import * as React from 'react';

import styles from './ResourceDialogForm.module.css';

interface ResourceDialogFormProps {
	readonly children: React.ReactNode;
	readonly id: string;
	readonly onSubmit: () => void;
	readonly submitDisabled: boolean;
}

export function ResourceDialogForm({
	children,
	id,
	onSubmit,
	submitDisabled,
}: ResourceDialogFormProps): React.ReactNode {
	return (
		<form
			className={styles['form']}
			id={id}
			onSubmit={(event) => {
				event.preventDefault();
				if (!submitDisabled) onSubmit();
			}}
		>
			{children}
		</form>
	);
}
