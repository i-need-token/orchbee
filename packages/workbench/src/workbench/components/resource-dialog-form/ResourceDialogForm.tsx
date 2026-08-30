import { Button, DialogFooter, Kbd } from '@orchbee/ui/components';
import * as React from 'react';

import styles from './ResourceDialogForm.module.css';

interface ResourceDialogFormProps {
	readonly cancelDisabled?: boolean | undefined;
	readonly cancelLabel: string;
	readonly children: React.ReactNode;
	readonly keyboardHints?: 'unambiguous-cancel-and-submit' | undefined;
	readonly onCancel: () => void;
	readonly onSubmit: () => void;
	readonly submitDisabled: boolean;
	readonly submitLabel: string;
}

export function ResourceDialogForm({
	cancelDisabled = false,
	cancelLabel,
	children,
	keyboardHints,
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
					{keyboardHints === 'unambiguous-cancel-and-submit' ? (
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Esc
						</Kbd>
					) : null}
				</Button>
				<Button disabled={submitDisabled} type='submit'>
					{submitLabel}
					{keyboardHints === 'unambiguous-cancel-and-submit' ? (
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					) : null}
				</Button>
			</DialogFooter>
		</form>
	);
}
