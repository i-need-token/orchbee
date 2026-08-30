import type { RemoteDirectoryBrowserLocation } from '../types/remote-directory-browser';

interface ProjectRemoteDirectoryBrowserOptions {
	readonly canNavigate: boolean;
	readonly canonicalPath: string;
	readonly directories: RemoteDirectoryBrowserLocation['directories'];
	readonly pathDraft: string;
	readonly pathEditorOpen: boolean;
	readonly showHiddenDirectories: boolean;
}

export interface RemoteDirectoryBrowserProjection {
	readonly canSubmitPath: boolean;
	readonly emptyState: 'empty' | 'hidden' | 'no-match' | null;
	readonly hasHiddenDirectories: boolean;
	readonly pathFilterState: 'exact' | 'inactive' | 'no-match' | 'partial';
	readonly visibleDirectories: RemoteDirectoryBrowserLocation['directories'];
}

export function projectRemoteDirectoryBrowser({
	canNavigate,
	canonicalPath,
	directories,
	pathDraft,
	pathEditorOpen,
	showHiddenDirectories,
}: ProjectRemoteDirectoryBrowserOptions): RemoteDirectoryBrowserProjection {
	const hasHiddenDirectories = directories.some((directory) => isHiddenDirectory(directory.name));
	const ordinarilyVisibleDirectories = showHiddenDirectories
		? directories
		: directories.filter((directory) => !isHiddenDirectory(directory.name));
	if (!pathEditorOpen) {
		return {
			canSubmitPath: canNavigate,
			emptyState: emptyState({
				hasHiddenDirectories,
				pathFilterState: 'inactive',
				showHiddenDirectories,
				visibleDirectories: ordinarilyVisibleDirectories,
			}),
			hasHiddenDirectories,
			pathFilterState: 'inactive',
			visibleDirectories: ordinarilyVisibleDirectories,
		};
	}

	const trimmedDraft = pathDraft.trim();
	const fragment = directChildFragment(canonicalPath, trimmedDraft);
	if (fragment === null || fragment.length === 0) {
		return {
			canSubmitPath: canNavigate,
			emptyState: emptyState({
				hasHiddenDirectories,
				pathFilterState: 'inactive',
				showHiddenDirectories,
				visibleDirectories: ordinarilyVisibleDirectories,
			}),
			hasHiddenDirectories,
			pathFilterState: 'inactive',
			visibleDirectories: ordinarilyVisibleDirectories,
		};
	}

	const candidatePool = fragment.startsWith('.') ? directories : ordinarilyVisibleDirectories;
	const normalizedFragment = fragment.toLowerCase();
	const visibleDirectories = candidatePool.filter((directory) =>
		directory.name.toLowerCase().startsWith(normalizedFragment),
	);
	const exactMatch = visibleDirectories.some((directory) => directory.path === trimmedDraft);
	const pathFilterState = exactMatch
		? 'exact'
		: visibleDirectories.length === 0
			? 'no-match'
			: 'partial';
	return {
		canSubmitPath: canNavigate && pathFilterState !== 'partial',
		emptyState: emptyState({
			hasHiddenDirectories,
			pathFilterState,
			showHiddenDirectories,
			visibleDirectories,
		}),
		hasHiddenDirectories,
		pathFilterState,
		visibleDirectories,
	};
}

function emptyState({
	hasHiddenDirectories,
	pathFilterState,
	showHiddenDirectories,
	visibleDirectories,
}: Pick<
	RemoteDirectoryBrowserProjection,
	'hasHiddenDirectories' | 'pathFilterState' | 'visibleDirectories'
> & {
	readonly showHiddenDirectories: boolean;
}): RemoteDirectoryBrowserProjection['emptyState'] {
	if (visibleDirectories.length > 0) return null;
	if (pathFilterState === 'no-match') return 'no-match';
	if (hasHiddenDirectories && !showHiddenDirectories) return 'hidden';
	return 'empty';
}

function directChildFragment(canonicalPath: string, pathDraft: string): string | null {
	const normalizedCanonicalPath =
		canonicalPath === '/' ? canonicalPath : canonicalPath.replace(/\/+$/u, '');
	const childPrefix = normalizedCanonicalPath === '/' ? '/' : `${normalizedCanonicalPath}/`;
	if (!pathDraft.startsWith(childPrefix)) return null;
	const fragment = pathDraft.slice(childPrefix.length);
	return fragment.includes('/') ? null : fragment;
}

function isHiddenDirectory(name: string): boolean {
	return name.startsWith('.') && name !== '.' && name !== '..';
}
