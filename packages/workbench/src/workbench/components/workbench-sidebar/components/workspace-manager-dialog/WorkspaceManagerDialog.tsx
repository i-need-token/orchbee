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

import type { WorkbenchResourceScope, WorkbenchWorkspace } from '../../../../../project-workspace';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-header/ResourceDialogHeader';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ProjectDirectoryList } from './components/project-directory-list/ProjectDirectoryList';
import { useWorkspaceDirectoryManager } from './hooks/useWorkspaceDirectoryManager';
import styles from './WorkspaceManagerDialog.module.css';

interface WorkspaceManagerDialogProps {
	readonly directoriesEditable: boolean;
	readonly directorySelectionPending?: boolean | undefined;
	readonly onAddDirectory: (path: string) => void;
	readonly onRemoveDirectory: (directoryId: string) => void;
	readonly onSelectSshDirectory?: (() => void) | undefined;
	readonly onSelectDirectory?: (() => void) | undefined;
	readonly onSetPrimaryDirectory: (directoryId: string) => void;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly name: string;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onPrimaryDirectoryChange: (directoryId: string) => void;
	readonly onSave: (name: string, primaryDirectoryId: string) => void;
	readonly primaryDirectoryId: string;
	readonly resources: WorkbenchResourceScope;
	readonly workspace: WorkbenchWorkspace;
}

export function WorkspaceManagerDialog({
	directoriesEditable,
	directorySelectionPending = false,
	onAddDirectory,
	onRemoveDirectory,
	onSelectSshDirectory,
	onSelectDirectory,
	onSetPrimaryDirectory,
	messages,
	name,
	onClose,
	onNameChange,
	onPrimaryDirectoryChange,
	onSave,
	primaryDirectoryId,
	resources,
	workspace,
}: WorkspaceManagerDialogProps): React.ReactNode {
	const formId = 'workbench-workspace-manager-form';
	const view = useWorkspaceDirectoryManager({ onAddDirectory, resources });
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
					description={
						directoriesEditable
							? messages.workspaceManageDescription
							: messages.workspaceManageNameOnlyDescription
					}
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
					{directoriesEditable && resources.directories.length > 1 && (
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
								{resources.directories.map((directory) => (
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
					)}
					{directoriesEditable && (
						<ProjectDirectoryList
							addingFolder={view.addingFolder}
							canAddFolder={view.canAddFolder}
							closeFolderInput={view.closeFolderInput}
							folderAlreadyAdded={view.folderAlreadyAdded}
							folderPath={view.folderPath}
							nativeSelectionPending={directorySelectionPending}
							messages={messages}
							onFolderPathKeyDown={view.onFolderPathKeyDown}
							onRemoveDirectory={onRemoveDirectory}
							onSetPrimaryDirectory={onSetPrimaryDirectory}
							onSelectFolder={onSelectDirectory}
							onSelectSshFolder={onSelectSshDirectory}
							resources={resources}
							setFolderPath={view.setFolderPath}
							showFolderInput={view.showFolderInput}
							submitFolder={view.submitFolder}
						/>
					)}
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
