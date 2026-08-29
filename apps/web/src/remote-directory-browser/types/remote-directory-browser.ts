export type RemoteDirectoryNavigationFailureReason =
	| 'invalid-path'
	| 'not-found'
	| 'not-directory'
	| 'permission-denied'
	| 'timed-out'
	| 'busy'
	| 'unavailable';

interface RemoteDirectoryBrowserCrumb {
	readonly label: string;
	readonly path: string;
}

interface RemoteDirectoryBrowserEntry {
	readonly name: string;
	readonly path: string;
}

export interface RemoteDirectoryBrowserLocation {
	readonly canonicalPath: string;
	readonly crumbs: readonly RemoteDirectoryBrowserCrumb[];
	readonly directories: readonly RemoteDirectoryBrowserEntry[];
	readonly navigationFailure: RemoteDirectoryNavigationFailureReason | null;
	readonly truncated: boolean;
}

export interface RemoteDirectoryBrowserProps {
	readonly busy: boolean;
	readonly canNavigate: boolean;
	readonly location: RemoteDirectoryBrowserLocation;
	readonly onCancel: () => void;
	readonly onNavigate: (path: string) => void;
	readonly onPathDraftChange: (path: string) => void;
	readonly onSelect: () => void;
	readonly pathDraft: string;
	readonly sourceLabel: string;
}
