import {
	Button,
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Input,
} from '@orchbee/ui/components';
import * as React from 'react';

import styles from './WorkbenchTabRenameDialog.module.css';

interface WorkbenchTabRenameDialogProps {
	readonly cancelLabel: string;
	readonly canSubmit: boolean;
	readonly confirmLabel: string;
	readonly description: string;
	readonly draftLabel: string;
	readonly inputLabel: string;
	readonly isOpen: boolean;
	readonly onCancel: () => void;
	readonly onDraftLabelChange: (label: string) => void;
	readonly onSubmit: () => void;
	readonly title: string;
}

export function WorkbenchTabRenameDialog({
	cancelLabel,
	canSubmit,
	confirmLabel,
	description,
	draftLabel,
	inputLabel,
	isOpen,
	onCancel,
	onDraftLabelChange,
	onSubmit,
	title,
}: WorkbenchTabRenameDialogProps): React.ReactNode {
	return (
		<Dialog
			open={isOpen}
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
		>
			<DialogContent showCloseButton={false}>
				<form
					className={styles['form']}
					onSubmit={(event) => {
						event.preventDefault();
						onSubmit();
					}}
				>
					<DialogHeader>
						<DialogTitle>{title}</DialogTitle>
						<DialogDescription>{description}</DialogDescription>
					</DialogHeader>
					<Input
						autoFocus
						aria-label={inputLabel}
						value={draftLabel}
						onChange={(event) => {
							onDraftLabelChange(event.target.value);
						}}
					/>
					<DialogFooter>
						<Button type='button' variant='outline' onClick={onCancel}>
							{cancelLabel}
						</Button>
						<Button disabled={!canSubmit} type='submit'>
							{confirmLabel}
						</Button>
					</DialogFooter>
				</form>
			</DialogContent>
		</Dialog>
	);
}
