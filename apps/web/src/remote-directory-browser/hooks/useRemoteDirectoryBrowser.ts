import { useId, useState } from 'react';

import type {
	RemoteDirectoryBrowserLocation,
	RemoteDirectoryNavigationFailureReason,
} from '../types/remote-directory-browser';
import { projectRemoteDirectoryBrowser } from '../utils/projectRemoteDirectoryBrowser';

interface UseRemoteDirectoryBrowserOptions {
	readonly busy: boolean;
	readonly canNavigate: boolean;
	readonly canonicalPath: string;
	readonly directories: RemoteDirectoryBrowserLocation['directories'];
	readonly navigationFailure: RemoteDirectoryNavigationFailureReason | null;
	readonly onNavigate: (path: string) => void;
	readonly onPathDraftChange: (path: string) => void;
	readonly pathDraft: string;
}

interface RemoteDirectoryBrowserViewController {
	readonly canSubmitPath: boolean;
	readonly hasHiddenDirectories: boolean;
	readonly hiddenDirectoriesVisible: boolean;
	readonly hiddenDirectoriesVisibilityId: string;
	readonly openPath: (path: string) => void;
	readonly openPathEditor: () => void;
	readonly pathFilterState: 'exact' | 'inactive' | 'no-match' | 'partial';
	readonly pathFormId: string;
	readonly pathGuidanceId: string;
	readonly pathInputId: string;
	readonly pathSubmitId: string;
	readonly resetPathEditor: () => void;
	readonly setHiddenDirectoriesVisible: (visible: boolean) => void;
	readonly showPathEditor: boolean;
	readonly submitPath: () => void;
	readonly visibleDirectories: RemoteDirectoryBrowserLocation['directories'];
}

export function useRemoteDirectoryBrowser({
	busy,
	canNavigate,
	canonicalPath,
	directories,
	navigationFailure,
	onNavigate,
	onPathDraftChange,
	pathDraft,
}: UseRemoteDirectoryBrowserOptions): RemoteDirectoryBrowserViewController {
	const [pathEditorOpen, setPathEditorOpen] = useState(false);
	const [failedPathEditorDismissed, setFailedPathEditorDismissed] = useState(false);
	const [hiddenDirectoriesVisible, setHiddenDirectoriesVisible] = useState(false);
	const componentId = useId();
	const showPathEditor =
		pathEditorOpen || (navigationFailure !== null && !failedPathEditorDismissed);
	const projection = projectRemoteDirectoryBrowser({
		canNavigate,
		canonicalPath,
		directories,
		pathDraft,
		pathEditorOpen: showPathEditor,
		showHiddenDirectories: hiddenDirectoriesVisible,
	});
	return {
		...projection,
		hiddenDirectoriesVisible,
		hiddenDirectoriesVisibilityId: `${componentId}-hidden-directories`,
		openPath: (path) => {
			setFailedPathEditorDismissed(false);
			setPathEditorOpen(false);
			onNavigate(path);
		},
		openPathEditor: () => {
			onPathDraftChange(canonicalPath.endsWith('/') ? canonicalPath : `${canonicalPath}/`);
			setFailedPathEditorDismissed(false);
			setPathEditorOpen(true);
		},
		resetPathEditor: () => {
			onPathDraftChange(canonicalPath);
			setFailedPathEditorDismissed(true);
			setPathEditorOpen(false);
		},
		setHiddenDirectoriesVisible,
		showPathEditor,
		submitPath: () => {
			if (projection.canSubmitPath && !busy) {
				setFailedPathEditorDismissed(false);
				setPathEditorOpen(false);
				onNavigate(pathDraft);
			}
		},
		pathFormId: `${componentId}-path-form`,
		pathGuidanceId: `${componentId}-path-guidance`,
		pathInputId: `${componentId}-path-input`,
		pathSubmitId: `${componentId}-path-submit`,
	};
}
