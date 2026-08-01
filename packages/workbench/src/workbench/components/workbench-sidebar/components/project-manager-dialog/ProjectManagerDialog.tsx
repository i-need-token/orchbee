import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	Field,
	FieldLabel,
	Input,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchProject } from '../../../../../project-workspace';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { ResourceDialogForm } from '../resource-dialog-form/ResourceDialogForm';
import { ProjectDirectoryList } from './components/project-directory-list/ProjectDirectoryList';
import { useProjectManagerDialog } from './hooks/useProjectManagerDialog';

interface ProjectManagerDialogProps {
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly name: string;
	readonly onAddDirectory: (path: string) => void;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onRemoveDirectory: (directoryId: string) => void;
	readonly onSave: (name: string) => void;
	readonly onSetPrimaryDirectory: (directoryId: string) => void;
	readonly project: WorkbenchProject;
}

export function ProjectManagerDialog({
	messages,
	name,
	onAddDirectory,
	onClose,
	onNameChange,
	onRemoveDirectory,
	onSave,
	onSetPrimaryDirectory,
	project,
}: ProjectManagerDialogProps): React.ReactNode {
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
				<ResourceDialogForm
					cancelLabel={messages.cancel}
					onCancel={onClose}
					onSubmit={() => onSave(normalizedName)}
					submitDisabled={normalizedName.length === 0}
					submitLabel={messages.save}
				>
					<DialogHeader>
						<DialogTitle>{messages.projectManageTitle}</DialogTitle>
						<DialogDescription>{messages.projectManageDescription}</DialogDescription>
					</DialogHeader>
					<Field>
						<FieldLabel htmlFor={`project-name-${project.id}`}>
							{messages.projectManageName}
						</FieldLabel>
						<Input
							id={`project-name-${project.id}`}
							placeholder={messages.projectManageNamePlaceholder}
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
						messages={messages}
						onFolderPathKeyDown={view.onFolderPathKeyDown}
						onRemoveDirectory={onRemoveDirectory}
						onSetPrimaryDirectory={onSetPrimaryDirectory}
						project={project}
						setFolderPath={view.setFolderPath}
						showFolderInput={view.showFolderInput}
						submitFolder={view.submitFolder}
					/>
				</ResourceDialogForm>
			</DialogContent>
		</Dialog>
	);
}
