import { Dialog, DialogContent, Field, FieldLabel, Input } from '@orchbee/ui/components';
import * as React from 'react';

import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-header/ResourceDialogHeader';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';

interface ProjectManagerDialogProps {
	readonly submitLabel?: string | undefined;
	readonly title?: string | undefined;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly name: string;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onSave: (name: string) => void;
}

export function ProjectManagerDialog({
	title,
	submitLabel,
	messages,
	name,
	onClose,
	onNameChange,
	onSave,
}: ProjectManagerDialogProps): React.ReactNode {
	const formId = 'workbench-project-manager-form';
	const nameId = React.useId();
	const normalizedName = name.trim();
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent closeLabel={messages.close} data-slot='workbench-project-manager'>
				<ResourceDialogHeader
					description={messages.projectManageDescription}
					title={title ?? messages.projectManageTitle}
				/>
				<ResourceDialogForm
					id={formId}
					onSubmit={() => onSave(normalizedName)}
					submitDisabled={normalizedName.length === 0}
				>
					<Field>
						<FieldLabel htmlFor={nameId}>{messages.projectManageName}</FieldLabel>
						<Input
							id={nameId}
							placeholder={messages.projectManageNamePlaceholder}
							required
							value={name}
							onChange={(event) => onNameChange(event.currentTarget.value)}
						/>
					</Field>
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelLabel={messages.cancel}
					formId={formId}
					onCancel={onClose}
					showCancelShortcut={false}
					showSubmitShortcut={false}
					submitDisabled={normalizedName.length === 0}
					submitLabel={submitLabel ?? messages.save}
				/>
			</DialogContent>
		</Dialog>
	);
}
