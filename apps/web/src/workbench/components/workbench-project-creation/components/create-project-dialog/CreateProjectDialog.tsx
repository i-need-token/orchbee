import {
	Dialog,
	DialogContent,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProjectCreationMessages } from '../../types/workbench-project-creation-messages';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-header/ResourceDialogHeader';
import styles from './CreateProjectDialog.module.css';

interface CreateProjectDialogProps {
	readonly messages: WorkbenchProjectCreationMessages;
	readonly onClose: () => void;
	readonly onPathChange: (path: string) => void;
	readonly onSubmit: (path: string) => void;
	readonly path: string;
	/** Why the Server refused the folder the field holds; it takes the help's place, as in the TUI. */
	readonly refusal: string | null;
	readonly title?: string | undefined;
}

export function CreateProjectDialog({
	messages,
	onClose,
	onPathChange,
	onSubmit,
	path,
	refusal,
	title,
}: CreateProjectDialogProps): React.ReactNode {
	const formId = 'workbench-project-create-form';
	const descriptionId = 'project-create-path-description';
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
					description={title === undefined ? messages.projectCreateDescription : undefined}
					title={title ?? messages.projectCreateTitle}
				/>
				<ResourceDialogForm
					id={formId}
					onSubmit={() => onSubmit(normalizedPath)}
					submitDisabled={normalizedPath.length === 0}
				>
					<Field>
						<FieldLabel htmlFor='project-create-path'>{messages.projectCreatePath}</FieldLabel>
						<Input
							aria-describedby={descriptionId}
							aria-invalid={refusal !== null || undefined}
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
						{refusal === null ? (
							<FieldDescription id={descriptionId}>
								{messages.projectCreatePathDescription}
							</FieldDescription>
						) : (
							<p className={styles['error']} id={descriptionId} role='alert'>
								{refusal}
							</p>
						)}
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
