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
import * as React from 'react';

import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';

interface CreateProjectDialogProps {
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly onClose: () => void;
	readonly onPathChange: (path: string) => void;
	readonly onSubmit: (path: string) => void;
	readonly path: string;
}

export function CreateProjectDialog({
	messages,
	onClose,
	onPathChange,
	onSubmit,
	path,
}: CreateProjectDialogProps): React.ReactNode {
	const normalizedPath = path.trim();
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent closeLabel={messages.close} data-slot='workbench-project-create'>
				<ResourceDialogForm
					cancelLabel={messages.cancel}
					keyboardHints='unambiguous-cancel-and-submit'
					onCancel={onClose}
					onSubmit={() => onSubmit(normalizedPath)}
					submitDisabled={normalizedPath.length === 0}
					submitLabel={messages.create}
				>
					<DialogHeader>
						<DialogTitle>{messages.projectCreateTitle}</DialogTitle>
						<DialogDescription>{messages.projectCreateDescription}</DialogDescription>
					</DialogHeader>
					<Field>
						<FieldLabel htmlFor='project-create-path'>{messages.projectCreatePath}</FieldLabel>
						<Input
							aria-describedby='project-create-path-description'
							autoFocus
							data-resource-dialog-value='code'
							id='project-create-path'
							placeholder={messages.projectCreatePathPlaceholder}
							value={path}
							onChange={(event) => onPathChange(event.currentTarget.value)}
						/>
						<FieldDescription id='project-create-path-description'>
							{messages.projectCreatePathDescription}
						</FieldDescription>
					</Field>
				</ResourceDialogForm>
			</DialogContent>
		</Dialog>
	);
}
