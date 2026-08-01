import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchProject } from '../../../../../../project-workspace';

interface UseProjectManagerDialogOptions {
	readonly onAddDirectory: (path: string) => void;
	readonly project: WorkbenchProject;
}

interface ProjectManagerDialogView {
	readonly addingFolder: boolean;
	readonly canAddFolder: boolean;
	readonly closeFolderInput: () => void;
	readonly folderAlreadyAdded: boolean;
	readonly folderPath: string;
	readonly onFolderPathKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly setFolderPath: (path: string) => void;
	readonly showFolderInput: () => void;
	readonly submitFolder: () => void;
}

export function useProjectManagerDialog({
	onAddDirectory,
	project,
}: UseProjectManagerDialogOptions): ProjectManagerDialogView {
	const [addingFolder, setAddingFolder] = React.useState(false);
	const [folderPath, setFolderPath] = React.useState('');
	const normalizedFolderPath = folderPath.trim();
	const folderAlreadyAdded = project.directories.some(
		(directory) => directory.path === normalizedFolderPath,
	);
	const canAddFolder = normalizedFolderPath.length > 0 && !folderAlreadyAdded;
	const closeFolderInput = useMemoizedFn(() => {
		setAddingFolder(false);
		setFolderPath('');
	});
	const showFolderInput = useMemoizedFn(() => setAddingFolder(true));
	const submitFolder = useMemoizedFn(() => {
		if (!canAddFolder) return;
		onAddDirectory(normalizedFolderPath);
		closeFolderInput();
	});
	const onFolderPathKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>) => {
		if (event.nativeEvent.isComposing) return;
		if (event.key === 'Enter') {
			event.preventDefault();
			submitFolder();
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			closeFolderInput();
		}
	});

	return {
		addingFolder,
		canAddFolder,
		closeFolderInput,
		folderAlreadyAdded,
		folderPath,
		onFolderPathKeyDown,
		setFolderPath,
		showFolderInput,
		submitFolder,
	};
}
