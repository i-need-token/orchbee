import {
	Dialog,
	DialogContent,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-form/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-form/ResourceDialogHeader';

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
	const formId = 'workbench-project-create-form';
	const normalizedPath = path.trim();
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent closeLabel={messages.close} data-slot='workbench-project-create'>
				<ResourceDialogHeader
					description={messages.projectCreateDescription}
					title={messages.projectCreateTitle}
				/>
				<ResourceDialogForm
					id={formId}
					onSubmit={() => onSubmit(normalizedPath)}
					submitDisabled={normalizedPath.length === 0}
				>
					<Field>
						<FieldLabel htmlFor='project-create-path'>{messages.projectCreatePath}</FieldLabel>
						<Input
							aria-describedby='project-create-path-description'
							autoCapitalize='none'
							autoComplete='off'
							autoCorrect='off'
							autoFocus
							data-code-like='true'
							id='project-create-path'
							placeholder={messages.projectCreatePathPlaceholder}
							required
							spellCheck={false}
							value={path}
							onChange={(event) => onPathChange(event.currentTarget.value)}
						/>
						<FieldDescription id='project-create-path-description'>
							{messages.projectCreatePathDescription}
						</FieldDescription>
					</Field>
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelLabel={messages.cancel}
					formId={formId}
					onCancel={onClose}
					submitDisabled={normalizedPath.length === 0}
					submitLabel={messages.create}
				/>
			</DialogContent>
		</Dialog>
	);
}
