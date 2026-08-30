import { Button, DialogFooter, Kbd } from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './ResourceDialogFooter.module.css';

interface ResourceDialogFooterProps {
	readonly cancelDisabled?: boolean | undefined;
	readonly cancelLabel: string;
	readonly formId: string;
	readonly onCancel: () => void;
	readonly showCancelShortcut?: boolean | undefined;
	readonly showSubmitShortcut?: boolean | undefined;
	readonly submitDisabled: boolean;
	readonly submitLabel: string;
}

export function ResourceDialogFooter({
	cancelDisabled = false,
	cancelLabel,
	formId,
	onCancel,
	showCancelShortcut = true,
	showSubmitShortcut = true,
	submitDisabled,
	submitLabel,
}: ResourceDialogFooterProps): React.ReactNode {
	return (
		<DialogFooter>
			<Button
				className={styles['action']}
				disabled={cancelDisabled}
				type='button'
				variant='outline'
				onClick={onCancel}
			>
				{cancelLabel}
				{showCancelShortcut ? (
					<Kbd aria-hidden='true' className={styles['shortcut']}>
						Esc
					</Kbd>
				) : null}
			</Button>
			<Button className={styles['action']} disabled={submitDisabled} form={formId} type='submit'>
				{submitLabel}
				{showSubmitShortcut ? (
					<Kbd aria-hidden='true' className={styles['shortcut']}>
						Enter
					</Kbd>
				) : null}
			</Button>
		</DialogFooter>
	);
}
