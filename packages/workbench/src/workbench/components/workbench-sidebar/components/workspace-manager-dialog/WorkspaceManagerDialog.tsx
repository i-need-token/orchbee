import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	Field,
	FieldLabel,
	Input,
	NativeSelect,
	NativeSelectOption,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProject, WorkbenchWorkspace } from '../../../../../project-workspace';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ResourceDialogForm } from '../resource-dialog-form/ResourceDialogForm';
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
	const normalizedName = name.trim();
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent closeLabel={messages.close} data-slot='workbench-workspace-manager'>
				<ResourceDialogForm
					cancelLabel={messages.cancel}
					onCancel={onClose}
					onSubmit={() => onSave(normalizedName, primaryDirectoryId)}
					submitDisabled={normalizedName.length === 0}
					submitLabel={messages.save}
				>
					<DialogHeader>
						<DialogTitle>{messages.workspaceManageTitle}</DialogTitle>
						<DialogDescription>{messages.workspaceManageDescription}</DialogDescription>
					</DialogHeader>
					<Field>
						<FieldLabel htmlFor={`workspace-name-${workspace.id}`}>
							{messages.workspaceManageName}
						</FieldLabel>
						<Input
							id={`workspace-name-${workspace.id}`}
							placeholder={messages.workspaceManageNamePlaceholder}
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
			</DialogContent>
		</Dialog>
	);
}
