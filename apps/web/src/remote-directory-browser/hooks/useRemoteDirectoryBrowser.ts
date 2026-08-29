import { useState } from 'react';

import type { RemoteDirectoryNavigationFailureReason } from '../types/remote-directory-browser';

interface UseRemoteDirectoryBrowserOptions {
	readonly busy: boolean;
	readonly canNavigate: boolean;
	readonly canonicalPath: string;
	readonly navigationFailure: RemoteDirectoryNavigationFailureReason | null;
	readonly onNavigate: (path: string) => void;
	readonly onPathDraftChange: (path: string) => void;
	readonly pathDraft: string;
}

interface RemoteDirectoryBrowserViewController {
	readonly openPath: (path: string) => void;
	readonly openPathEditor: () => void;
	readonly resetPathEditor: () => void;
	readonly showPathEditor: boolean;
	readonly submitPath: () => void;
}

export function useRemoteDirectoryBrowser({
	busy,
	canNavigate,
	canonicalPath,
	navigationFailure,
	onNavigate,
	onPathDraftChange,
	pathDraft,
}: UseRemoteDirectoryBrowserOptions): RemoteDirectoryBrowserViewController {
	const [pathEditorOpen, setPathEditorOpen] = useState(false);
	return {
		openPath: (path) => {
			setPathEditorOpen(false);
			onNavigate(path);
		},
		openPathEditor: () => {
			onPathDraftChange(canonicalPath);
			setPathEditorOpen(true);
		},
		resetPathEditor: () => {
			onPathDraftChange(canonicalPath);
			setPathEditorOpen(false);
		},
		showPathEditor: pathEditorOpen || navigationFailure !== null,
		submitPath: () => {
			if (canNavigate && !busy) {
				setPathEditorOpen(false);
				onNavigate(pathDraft);
			}
		},
	};
}
