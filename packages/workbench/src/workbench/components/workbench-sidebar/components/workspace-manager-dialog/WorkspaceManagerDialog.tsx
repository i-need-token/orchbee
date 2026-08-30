import {
	Dialog,
	DialogContent,
	Field,
	FieldLabel,
	Input,
	NativeSelect,
	NativeSelectOption,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProject, WorkbenchWorkspace } from '../../../../../project-workspace';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-form/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-form/ResourceDialogHeader';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import styles from './WorkspaceManagerDialog.module.css';

interface WorkspaceManagerDialogProps {
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly name: string;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onPrimaryDirectoryChange: (directoryId: string) => void;
	readonly onSave: (name: string, primaryDirectoryId: string) => void;
	readonly primaryDirectoryId: string;
	readonly project: WorkbenchProject;
	readonly workspace: WorkbenchWorkspace;
}

export function WorkspaceManagerDialog({
	messages,
	name,
	onClose,
	onNameChange,
	onPrimaryDirectoryChange,
	onSave,
	primaryDirectoryId,
	project,
	workspace,
}: WorkspaceManagerDialogProps): React.ReactNode {
	const formId = 'workbench-workspace-manager-form';
	const normalizedName = name.trim();
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent closeLabel={messages.close} data-slot='workbench-workspace-manager'>
				<ResourceDialogHeader
					description={messages.workspaceManageDescription}
					title={messages.workspaceManageTitle}
				/>
				<ResourceDialogForm
					id={formId}
					onSubmit={() => onSave(normalizedName, primaryDirectoryId)}
					submitDisabled={normalizedName.length === 0}
				>
					<Field>
						<FieldLabel htmlFor={`workspace-name-${workspace.id}`}>
							{messages.workspaceManageName}
						</FieldLabel>
						<Input
							id={`workspace-name-${workspace.id}`}
							placeholder={messages.workspaceManageNamePlaceholder}
							required
							value={name}
							onChange={(event) => onNameChange(event.currentTarget.value)}
						/>
					</Field>
					<Field>
						<FieldLabel htmlFor={`workspace-directory-${workspace.id}`}>
							{messages.workspaceManagePrimaryFolder}
						</FieldLabel>
						<NativeSelect
							className={styles['directorySelect']}
							id={`workspace-directory-${workspace.id}`}
							value={primaryDirectoryId}
							onChange={(event) => onPrimaryDirectoryChange(event.currentTarget.value)}
						>
							{project.directories.map((directory) => (
								<NativeSelectOption
									key={directory.id}
									disabled={!directory.available}
									value={directory.id}
								>
									{directory.name}
								</NativeSelectOption>
							))}
						</NativeSelect>
					</Field>
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelLabel={messages.cancel}
					formId={formId}
					onCancel={onClose}
					showCancelShortcut={false}
					showSubmitShortcut={false}
					submitDisabled={normalizedName.length === 0}
					submitLabel={messages.save}
				/>
			</DialogContent>
		</Dialog>
	);
}
