import {
	Dialog,
	DialogContent,
	Field,
	FieldDescription,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type { NewProjectFolderState } from '../../../../model/transitionNewProjectFolder';
import type { WorkbenchProjectCreationMessages } from '../../types/workbench-project-creation-messages';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-header/ResourceDialogHeader';
import styles from './CreateProjectFolderDialog.module.css';

interface CreateProjectFolderDialogProps {
	readonly messages: WorkbenchProjectCreationMessages;
	readonly state: Extract<NewProjectFolderState, { readonly name: string }>;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onRecover: () => void;
	readonly onSubmit: () => void;
}

export function CreateProjectFolderDialog({
	messages,
	state,
	onClose,
	onNameChange,
	onRecover,
	onSubmit,
}: CreateProjectFolderDialogProps): React.ReactNode {
	const pending = state.kind === 'creating' || state.kind === 'opening';
	const recovery = state.kind === 'recovery';
	const formId = 'workbench-new-project-folder-form';
	const disabled = pending || (!recovery && state.name.trim().length === 0);
	return (
		<Dialog
			open
			onOpenChange={(open) => {
				if (!open && !pending) onClose();
			}}
		>
			<DialogContent
				showCloseButton={!pending}
				closeLabel={messages.close}
				data-slot='workbench-project-folder-create'
			>
				<ResourceDialogHeader
					title={messages.newFolderTitle}
					description={messages.newFolderDescription}
				/>
				<ResourceDialogForm
					id={formId}
					onSubmit={recovery ? onRecover : onSubmit}
					submitDisabled={disabled}
				>
					<dl className={styles['metadata']}>
						<dt>{messages.newFolderParent}</dt>
						<dd>
							<code>{state.parent.path}</code>
						</dd>
					</dl>
					{recovery ? (
						<div className={styles['feedback']} role='alert'>
							<p>{state.path === null ? messages.newFolderUnknown : messages.newFolderRetained}</p>
							<code>{state.path ?? state.name}</code>
						</div>
					) : (
						<Field>
							<FieldLabel htmlFor='new-project-folder-name'>{messages.newFolderName}</FieldLabel>
							<Input
								aria-describedby='new-project-folder-name-description'
								autoCapitalize='none'
								autoComplete='off'
								autoCorrect='off'
								autoFocus
								data-code-like='true'
								disabled={pending}
								id='new-project-folder-name'
								maxLength={255}
								onChange={(event) => onNameChange(event.currentTarget.value)}
								required
								spellCheck={false}
								value={state.name}
							/>
							<FieldDescription id='new-project-folder-name-description'>
								{messages.newFolderNameHint}
							</FieldDescription>
						</Field>
					)}
					{state.kind === 'draft' && state.failure !== null ? (
						<p className={styles['feedback']} role='alert'>
							{messages.folderAcquisitionFailureMessages[state.failure]}
						</p>
					) : null}
					{pending ? (
						<p className={styles['status']} role='status'>
							{state.kind === 'creating' ? messages.newFolderCreating : messages.newFolderOpening}
						</p>
					) : null}
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelDisabled={pending}
					cancelLabel={recovery ? messages.close : messages.cancel}
					formId={formId}
					onCancel={onClose}
					showCancelShortcut={!pending}
					showSubmitShortcut={!pending}
					submitDisabled={disabled}
					submitLabel={recovery ? messages.selectFolder : messages.create}
				/>
			</DialogContent>
		</Dialog>
	);
}
