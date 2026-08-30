import { Dialog, DialogContent, Field, FieldLabel, Input } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProject } from '../../../../../project-workspace';
import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-form/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-form/ResourceDialogHeader';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ProjectDirectoryList } from './components/project-directory-list/ProjectDirectoryList';
import { useProjectManagerDialog } from './hooks/useProjectManagerDialog';

interface ProjectManagerDialogProps {
	readonly directorySelectionPending?: boolean | undefined;
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly name: string;
	readonly onAddDirectory: (path: string) => void;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onRemoveDirectory: (directoryId: string) => void;
	readonly onSelectSshDirectory?: (() => void) | undefined;
	readonly onSave: (name: string) => void;
	readonly onSelectDirectory?: (() => void) | undefined;
	readonly onSetPrimaryDirectory: (directoryId: string) => void;
	readonly project: WorkbenchProject;
}

export function ProjectManagerDialog({
	directorySelectionPending = false,
	messages,
	name,
	onAddDirectory,
	onClose,
	onNameChange,
	onRemoveDirectory,
	onSelectSshDirectory,
	onSave,
	onSelectDirectory,
	onSetPrimaryDirectory,
	project,
}: ProjectManagerDialogProps): React.ReactNode {
	const formId = 'workbench-project-manager-form';
	const view = useProjectManagerDialog({ onAddDirectory, project });
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
					title={messages.projectManageTitle}
				/>
				<ResourceDialogForm
					id={formId}
					onSubmit={() => onSave(normalizedName)}
					submitDisabled={normalizedName.length === 0}
				>
					<Field>
						<FieldLabel htmlFor={`project-name-${project.id}`}>
							{messages.projectManageName}
						</FieldLabel>
						<Input
							id={`project-name-${project.id}`}
							placeholder={messages.projectManageNamePlaceholder}
							required
							value={name}
							onChange={(event) => onNameChange(event.currentTarget.value)}
						/>
					</Field>
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
						project={project}
						setFolderPath={view.setFolderPath}
						showFolderInput={view.showFolderInput}
						submitFolder={view.submitFolder}
					/>
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
