import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { CreateProjectDialog } from './components/create-project-dialog/CreateProjectDialog';
import { CreateProjectFolderDialog } from './components/create-project-folder-dialog/CreateProjectFolderDialog';
import type { WorkbenchProjectCreationView } from './hooks/useWorkbenchProjectCreation';
import type { WorkbenchProjectCreationMessages } from './types/workbench-project-creation-messages';
import styles from './WorkbenchProjectCreation.module.css';

interface WorkbenchProjectCreationProps {
	readonly messages: WorkbenchProjectCreationMessages;
	readonly view: WorkbenchProjectCreationView;
}

/** The dialogs of a Project started from Search: a folder path, a new folder, or why it failed. */
export function WorkbenchProjectCreation({
	messages,
	view: { dismissFailure, failure, newFolder, pathForm },
}: WorkbenchProjectCreationProps): ReactNode {
	return (
		<>
			{pathForm === null ? null : (
				<CreateProjectDialog
					messages={messages}
					onClose={pathForm.close}
					onPathChange={pathForm.setPath}
					onSubmit={pathForm.submit}
					path={pathForm.path}
				/>
			)}
			{'parent' in newFolder.state ? (
				<CreateProjectFolderDialog
					messages={messages}
					onClose={newFolder.close}
					onNameChange={newFolder.setName}
					onRecover={newFolder.recover}
					onSubmit={newFolder.submit}
					state={newFolder.state}
				/>
			) : null}
			{failure === null ? null : (
				<AlertDialog
					onOpenChange={(open) => {
						if (!open) dismissFailure();
					}}
					open
				>
					<AlertDialogContent
						className={styles['alertContent']}
						data-slot='workbench-project-folder-acquisition-failure'
					>
						<AlertDialogHeader className={styles['alertHeader']}>
							<AlertDialogTitle className={styles['alertTitle']}>
								{messages.folderAcquisitionFailureTitle}
							</AlertDialogTitle>
							<AlertDialogDescription className={styles['alertDescription']}>
								{messages.folderAcquisitionFailureMessages[failure]}
							</AlertDialogDescription>
						</AlertDialogHeader>
						<AlertDialogFooter className={styles['alertFooter']}>
							<AlertDialogAction onClick={dismissFailure}>
								{messages.folderAcquisitionFailureDismiss}
							</AlertDialogAction>
						</AlertDialogFooter>
					</AlertDialogContent>
				</AlertDialog>
			)}
		</>
	);
}
