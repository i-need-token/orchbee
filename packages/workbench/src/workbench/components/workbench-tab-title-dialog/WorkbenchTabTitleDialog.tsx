import {
	Dialog,
	DialogContent,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import { useRef, type ReactNode } from 'react';

import { ResourceDialogForm } from '../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../resource-dialog-header/ResourceDialogHeader';

interface WorkbenchTabTitleDialogMessages {
	readonly cancel: string;
	readonly close: string;
	readonly description: string;
	readonly field: string;
	readonly placeholder: string;
	/** Null when emptying the field cannot bring back the title the terminal reports. */
	readonly restoreDescription: string | null;
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
	const formId = 'workbench-tab-title-form';
	const inputId = 'workbench-tab-title-input';
	// Not `autoFocus`: the menu this can open from returns focus as it closes, after that has run.
	const input = useRef<HTMLInputElement>(null);
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
				initialFocus={input}
			>
				<ResourceDialogHeader description={messages.description} title={messages.title} />
				<ResourceDialogForm id={formId} onSubmit={onSubmit} submitDisabled={false}>
					<Field>
						<FieldLabel htmlFor={inputId}>{messages.field}</FieldLabel>
						<Input
							aria-describedby={messages.restoreDescription === null ? undefined : descriptionId}
							id={inputId}
							placeholder={messages.placeholder}
							ref={input}
							value={title}
							onChange={(event) => onTitleChange(event.currentTarget.value)}
							onFocus={(event) => event.currentTarget.select()}
						/>
						{messages.restoreDescription === null ? null : (
							<FieldDescription id={descriptionId}>{messages.restoreDescription}</FieldDescription>
						)}
					</Field>
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelLabel={messages.cancel}
					formId={formId}
					onCancel={onCancel}
					submitDisabled={false}
					submitLabel={messages.save}
				/>
			</DialogContent>
		</Dialog>
	);
}
