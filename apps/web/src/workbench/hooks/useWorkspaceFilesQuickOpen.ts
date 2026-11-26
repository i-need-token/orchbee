import {
	createTypedActionRuntime,
	type ActionRuntime,
	type PreparedActionInvocation,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import { registerWorkbenchWorkspaceFileOpenAction } from '../actions/registerWorkbenchWorkspaceFileOpenAction';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchWorkspaceFileOpenInput,
} from '../actions/workbench-action-ids';
import type { WorkbenchSearchFilter } from '../types/workbench-search';
import type {
	WorkbenchSearchProvider,
	WorkbenchSearchProviderResult,
	WorkbenchSearchTextRange,
} from '../types/workbench-search-provider';
import type {
	WorkbenchWorkspaceFilesSource,
	WorkspaceFileOpenRequest,
} from '../types/workbench-workspace-files';

interface UseWorkspaceFilesQuickOpenOptions {
	readonly runtime: ActionRuntime;
	/** Where files are read; without it quick open offers nothing. */
	readonly source: WorkbenchWorkspaceFilesSource | undefined;
	/** Whether Search shows; its file list is let go once it closes. */
	readonly searchOpen: boolean;
	readonly workspaceId: string | null;
}

interface WorkspaceFilesQuickOpenView {
	readonly providers: readonly WorkbenchSearchProvider[];
	/** The file the right bar opens next; null until one is asked for. */
	readonly openRequest: WorkspaceFileOpenRequest | null;
	/** What the right bar reports it opened, which quick open lists first while the window lives. */
	readonly onFileOpened: (workspaceId: string, path: string) => void;
}

const FILES_FILTERS: readonly WorkbenchSearchFilter[] = ['files'];
const NO_PROVIDERS: readonly WorkbenchSearchProvider[] = [];
/** The rows one search shows, as many as a screen of quick open scrolls through. */
const RESULT_LIMIT = 50;
/** The files a Workspace lists as recently opened. */
const RECENT_LIMIT = 20;

/** The right bar's code, which matches file names too; it loads once, with the right bar. */
const loadWorkspaceFiles = () => import('../../workspace-files-surface');
type WorkspaceFileFinder = InstanceType<
	Awaited<ReturnType<typeof loadWorkspaceFiles>>['WorkspaceFileFinder']
>;

interface HeldFinder {
	readonly workspaceId: string;
	readonly finder: WorkspaceFileFinder;
}

interface ShownFile {
	readonly path: string;
	readonly name: string;
	readonly folder: string;
	readonly nameMatches?: readonly WorkbenchSearchTextRange[] | undefined;
	readonly folderMatches?: readonly WorkbenchSearchTextRange[] | undefined;
}

function shownFile(path: string): ShownFile {
	const slash = path.lastIndexOf('/');
	return { folder: slash < 0 ? '' : path.slice(0, slash), name: path.slice(slash + 1), path };
}

/**
 * Quick open: the Files filter of Search, which finds a Workspace file by name and opens it in the
 * right bar's float window. Its matching loads with the right bar's code the first time it is used,
 * and the Workspace's file list is read while Search shows it and let go when Search closes.
 */
export function useWorkspaceFilesQuickOpen({
	runtime,
	source,
	searchOpen,
	workspaceId,
}: UseWorkspaceFilesQuickOpenOptions): WorkspaceFilesQuickOpenView {
	const intl = useOrchbeeIntl();
	const [openRequest, setOpenRequest] = React.useState<WorkspaceFileOpenRequest | null>(null);
	const [recents] = React.useState(() => new Map<string, readonly string[]>());
	const [listeners] = React.useState(() => new Set<() => void>());
	const held = React.useRef<HeldFinder | null>(null);
	// Its own scope, so preparing a row reads this Action alone rather than every one the window has.
	const scope = React.useMemo(
		() => runtime.createScope('workbench.workspace-files.quick-open'),
		[runtime],
	);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
		[runtime],
	);

	const open = useMemoizedFn((input: WorkbenchWorkspaceFileOpenInput) => {
		setOpenRequest((previous) => ({
			...input,
			serial: (previous?.serial ?? 0) + 1,
		}));
	});
	React.useEffect(() => {
		if (source === undefined) return undefined;
		const registration = registerWorkbenchWorkspaceFileOpenAction({
			activeWorkspaceId: workspaceId,
			open,
			scope,
			typedRuntime,
		});
		return () => registration.dispose();
	}, [open, scope, source, typedRuntime, workspaceId]);

	const release = useMemoizedFn(() => {
		held.current?.finder.close();
		held.current = null;
	});
	React.useEffect(() => {
		if (!searchOpen) release();
	}, [release, searchOpen]);
	React.useEffect(() => release, [release]);

	const onFileOpened = useMemoizedFn((openedIn: string, path: string) => {
		const earlier = recents.get(openedIn) ?? [];
		recents.set(
			openedIn,
			[path, ...earlier.filter((recent) => recent !== path)].slice(0, RECENT_LIMIT),
		);
	});

	const provider = React.useMemo<WorkbenchSearchProvider | null>(() => {
		if (source === undefined || workspaceId === null) return null;
		const prepare = (input: WorkbenchWorkspaceFileOpenInput): PreparedActionInvocation | null => {
			const discovered = runtime
				.discover(
					{ scope, source: 'command-surface' },
					{ inputByActionId: { [WORKBENCH_ACTION_IDS.openWorkspaceFile]: input } },
				)
				.find((result) => result.descriptor.id === WORKBENCH_ACTION_IDS.openWorkspaceFile);
			return discovered?.kind === 'ready' ? discovered.invocation : null;
		};
		const resultFor = (
			file: ShownFile,
			location: Pick<WorkbenchWorkspaceFileOpenInput, 'column' | 'line'>,
		): WorkbenchSearchProviderResult[] => {
			const prepared = prepare({ path: file.path, workspaceId, ...location });
			if (prepared === null) return [];
			return [
				{
					availability: { kind: 'enabled' },
					canonicalLabel: file.path,
					...(file.folder.length === 0 ? {} : { description: file.folder }),
					...(file.folderMatches === undefined || file.folderMatches.length === 0
						? {}
						: { descriptionMatches: file.folderMatches }),
					icon: 'file',
					id: file.path,
					invocation: { kind: 'ready', prepared },
					keywords: [],
					label: file.name,
					...(file.nameMatches === undefined || file.nameMatches.length === 0
						? {}
						: { labelMatches: file.nameMatches }),
				},
			];
		};
		const finderFor = (module: Awaited<ReturnType<typeof loadWorkspaceFiles>>) => {
			const current = held.current;
			if (current?.workspaceId === workspaceId && !current.finder.failed) return current.finder;
			current?.finder.close();
			const finder = new module.WorkspaceFileFinder(source, workspaceId);
			finder.subscribe(() => {
				// A copy, because a listener that runs Search again subscribes anew while this loops.
				for (const listener of Array.from(listeners)) listener();
			});
			held.current = { finder, workspaceId };
			return finder;
		};
		return {
			filters: FILES_FILTERS,
			getDefaultSections: () => {
				const paths = recents.get(workspaceId) ?? [];
				const results = paths.flatMap((path) => resultFor(shownFile(path), {}));
				return results.length === 0
					? []
					: [
							{
								id: 'recent',
								label: intl.formatMessage(workbenchMessages.searchGroupRecentFiles),
								order: 0,
								results,
							},
						];
			},
			id: 'workspace-files',
			label: intl.formatMessage(workbenchMessages.searchProviderFiles),
			priority: 0,
			resultLimit: RESULT_LIMIT,
			search: async (query, _context, signal) => {
				const module = await loadWorkspaceFiles();
				if (signal.aborted) return [];
				const finder = finderFor(module);
				await finder.ready(signal);
				if (signal.aborted) return [];
				if (finder.failed) throw new Error('The Workspace files could not be listed');
				const found = finder.find(query, RESULT_LIMIT);
				const location = {
					...(found.line === undefined ? {} : { line: found.line }),
					...(found.column === undefined ? {} : { column: found.column }),
				};
				const results = found.matches.flatMap((match) => resultFor(match, location));
				return results.length === 0
					? []
					: [
							{
								id: 'files',
								label: intl.formatMessage(workbenchMessages.searchGroupFiles),
								order: 0,
								results,
							},
						];
			},
			subscribe: (listener) => {
				listeners.add(listener);
				return {
					dispose: () => {
						listeners.delete(listener);
					},
				};
			},
		};
	}, [intl, listeners, recents, runtime, scope, source, workspaceId]);

	const providers = React.useMemo(
		() => (provider === null ? NO_PROVIDERS : [provider]),
		[provider],
	);
	return { onFileOpened, openRequest, providers };
}
