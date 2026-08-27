import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { ResourceDialogForm } from '../resource-dialog-form/ResourceDialogForm';

interface WorkbenchTabTitleDialogMessages {
	readonly cancel: string;
	readonly close: string;
	readonly description: string;
	readonly field: string;
	readonly placeholder: string;
	readonly restoreDescription: string;
	readonly save: string;
	readonly title: string;
}

interface WorkbenchTabTitleDialogProps {
	readonly finalFocus: () => HTMLElement | null;
	readonly messages: WorkbenchTabTitleDialogMessages;
	readonly onCancel: () => void;
	readonly onSubmit: () => void;
	readonly onTitleChange: (title: string) => void;
	readonly title: string;
}

export function WorkbenchTabTitleDialog({
	finalFocus,
	messages,
	onCancel,
	onSubmit,
	onTitleChange,
	title,
}: WorkbenchTabTitleDialogProps): ReactNode {
	const descriptionId = 'workbench-tab-title-description';
	const inputId = 'workbench-tab-title-input';
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onCancel();
			}}
			open
		>
			<DialogContent
				closeLabel={messages.close}
				data-slot='workbench-tab-title-dialog'
				finalFocus={finalFocus}
			>
				<ResourceDialogForm
					cancelLabel={messages.cancel}
					onCancel={onCancel}
					onSubmit={onSubmit}
					submitDisabled={false}
					submitLabel={messages.save}
				>
					<DialogHeader>
						<DialogTitle>{messages.title}</DialogTitle>
						<DialogDescription>{messages.description}</DialogDescription>
					</DialogHeader>
					<Field>
						<FieldLabel htmlFor={inputId}>{messages.field}</FieldLabel>
						<Input
							aria-describedby={descriptionId}
							autoFocus
							id={inputId}
							placeholder={messages.placeholder}
							value={title}
							onChange={(event) => onTitleChange(event.currentTarget.value)}
							onFocus={(event) => event.currentTarget.select()}
						/>
						<FieldDescription id={descriptionId}>{messages.restoreDescription}</FieldDescription>
					</Field>
				</ResourceDialogForm>
			</DialogContent>
		</Dialog>
	);
}
