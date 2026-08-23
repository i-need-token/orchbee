import { Button, DialogFooter, Kbd } from '@orchbee/ui/components';
import * as React from 'react';

import styles from './ResourceDialogForm.module.css';

interface ResourceDialogFormProps {
	readonly cancelDisabled?: boolean | undefined;
	readonly cancelLabel: string;
	readonly children: React.ReactNode;
	readonly onCancel: () => void;
	readonly onSubmit: () => void;
	readonly submitDisabled: boolean;
	readonly submitLabel: string;
}

export function ResourceDialogForm({
	cancelDisabled = false,
	cancelLabel,
	children,
	onCancel,
	onSubmit,
	submitDisabled,
	submitLabel,
}: ResourceDialogFormProps): React.ReactNode {
	return (
		<form
			className={styles['form']}
			onSubmit={(event) => {
				event.preventDefault();
				if (!submitDisabled) onSubmit();
			}}
		>
			{children}
			<DialogFooter>
				<Button disabled={cancelDisabled} type='button' variant='outline' onClick={onCancel}>
					{cancelLabel}
					<Kbd aria-hidden='true' className={styles['shortcut']}>
						Esc
					</Kbd>
				</Button>
				<Button disabled={submitDisabled} type='submit'>
					{submitLabel}
					<Kbd aria-hidden='true' className={styles['shortcut']}>
						Enter
					</Kbd>
				</Button>
			</DialogFooter>
		</form>
	);
}
