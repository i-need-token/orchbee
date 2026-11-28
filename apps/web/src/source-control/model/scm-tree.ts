import type { VirtualTreeSource } from '../../virtual-tree';
import {
	SCM_GROUP_IDS,
	type ScmGroupId,
	type ScmResource,
	type ScmResourceGroups,
	type ScmResourceStatus,
} from './scm-resource';
import type { ScmViewMode, ScmViewSortKey } from './scm-view-state';

/** One repository as the Changes tree shows it. */
interface ScmTreeRepository {
	/** The groups at the repository's latest read status; null before the first. */
	readonly groups: ScmResourceGroups | null;
	readonly repository: string;
}

/** What the Changes tree shows now: the visible repositories, in order, and how to list rows. */
export interface ScmTreeSnapshot {
	readonly options: ScmTreeOptions;
	readonly repositories: readonly ScmTreeRepository[];
}

/** A row of the Changes tree, or the tree's input. */
export type ScmTreeItem =
	| { readonly kind: 'workspace' }
	| { readonly kind: 'repository'; readonly repository: string }
	| {
			readonly group: ScmGroupId;
			readonly kind: 'group';
			readonly repository: string;
			readonly resources: readonly ScmResource[];
	  }
	| {
			readonly children: readonly ScmTreeItem[];
			readonly group: ScmGroupId;
			readonly kind: 'folder';
			/** The folders a compact row stands for, outermost first; one name without compaction. */
			readonly names: readonly string[];
			/** The innermost folder's path. */
			readonly path: string;
			readonly repository: string;
	  }
	| {
			readonly kind: 'resource';
			readonly name: string;
			readonly repository: string;
			readonly resource: ScmResource;
	  };

interface ScmTreeOptions {
	/** VS Code's `compareFileNames` for the locale: numeric, ignoring case and accents. */
	readonly compareNames: (one: string, other: string) => number;
	readonly mode: ScmViewMode;
	/** The Worker's separator. */
	readonly separator: '/' | '\\';
	/** The list's order; a tree always sorts by name, folders first. */
	readonly sortKey: ScmViewSortKey;
	/** A status's hover text, which Sort by Status compares as VS Code does. */
	readonly statusText: (status: ScmResourceStatus) => string;
}

/** The tree's input, whose children are the top-level rows. */
export const SCM_TREE_INPUT: ScmTreeItem = { kind: 'workspace' };

/** The last segment of a path. */
function nameOf(path: string, separator: string): string {
	return path.slice(path.lastIndexOf(separator) + 1);
}

/** VS Code's `comparePaths`: folders by lowercased code units, the last segment by name. */
function comparePaths(
	one: string,
	other: string,
	separator: string,
	compareNames: ScmTreeOptions['compareNames'],
): number {
	const oneParts = one.split(separator);
	const otherParts = other.split(separator);
	for (let index = 0; ; index += 1) {
		const oneEnds = index === oneParts.length - 1;
		const otherEnds = index === otherParts.length - 1;
		const onePart = oneParts[index] ?? '';
		const otherPart = otherParts[index] ?? '';
		if (oneEnds && otherEnds) return compareNames(onePart, otherPart);
		if (oneEnds) return -1;
		if (otherEnds) return 1;
		const oneFolder = onePart.toLowerCase();
		const otherFolder = otherPart.toLowerCase();
		if (oneFolder !== otherFolder) return oneFolder < otherFolder ? -1 : 1;
	}
}

function listItems(
	repository: string,
	resources: readonly ScmResource[],
	options: ScmTreeOptions,
): ScmTreeItem[] {
	const { compareNames, separator, sortKey, statusText } = options;
	const items = resources.map((resource) => ({
		kind: 'resource' as const,
		name: nameOf(resource.path, separator),
		repository,
		resource,
	}));
	return items.sort((one, other) => {
		if (sortKey === 'name') return compareNames(one.name, other.name);
		if (sortKey === 'status') {
			const oneText = statusText(one.resource.status);
			const otherText = statusText(other.resource.status);
			if (oneText !== otherText) return oneText < otherText ? -1 : 1;
		}
		return comparePaths(one.resource.path, other.resource.path, separator, compareNames);
	});
}

interface FolderNode {
	readonly files: ScmResource[];
	readonly folders: Map<string, FolderNode>;
	readonly path: string;
}

function folderNode(path: string): FolderNode {
	return { files: [], folders: new Map(), path };
}

/** The items of one folder: folders first, then files, each by name; single-folder chains merge. */
function folderItems(
	node: FolderNode,
	repository: string,
	group: ScmGroupId,
	options: ScmTreeOptions,
): ScmTreeItem[] {
	const { compareNames, separator } = options;
	const folders = [...node.folders.entries()].sort(([one], [other]) => compareNames(one, other));
	const items: ScmTreeItem[] = folders.map(([name, child]) => {
		const names = [name];
		let innermost = child;
		// A folder whose only entry is a folder merges with it, as `scm.compactFolders` does.
		while (innermost.files.length === 0 && innermost.folders.size === 1) {
			const only = innermost.folders.entries().next();
			if (only.done === true) break;
			const [nextName, next] = only.value;
			names.push(nextName);
			innermost = next;
		}
		return {
			children: folderItems(innermost, repository, group, options),
			group,
			kind: 'folder',
			names,
			path: innermost.path,
			repository,
		};
	});
	const files = node.files
		.map((resource) => ({ name: nameOf(resource.path, separator), resource }))
		.sort((one, other) => compareNames(one.name, other.name));
	for (const { name, resource } of files)
		items.push({ kind: 'resource', name, repository, resource });
	return items;
}

function treeItems(
	repository: string,
	group: ScmGroupId,
	resources: readonly ScmResource[],
	options: ScmTreeOptions,
): ScmTreeItem[] {
	const { separator } = options;
	const root = folderNode(repository);
	const prefix = repository.endsWith(separator) ? repository : `${repository}${separator}`;
	for (const resource of resources) {
		const relative = resource.path.startsWith(prefix)
			? resource.path.slice(prefix.length)
			: nameOf(resource.path, separator);
		const segments = relative.split(separator);
		segments.pop();
		let node = root;
		for (const segment of segments) {
			let child = node.folders.get(segment);
			if (child === undefined) {
				child = folderNode(
					`${node === root ? prefix.slice(0, -1) : node.path}${separator}${segment}`,
				);
				node.folders.set(segment, child);
			}
			node = child;
		}
		node.files.push(resource);
	}
	return folderItems(root, repository, group, options);
}

/** A row's identity: unique across the tree, so one path can show in two groups. */
function scmTreeItemKey(item: ScmTreeItem): string {
	switch (item.kind) {
		case 'workspace':
			return 'workspace';
		case 'repository':
			return `repository\0${item.repository}`;
		case 'group':
			return `group\0${item.repository}\0${item.group}`;
		case 'folder':
			return `folder\0${item.repository}\0${item.group}\0${item.path}`;
		case 'resource':
			return `resource\0${item.repository}\0${item.resource.group}\0${item.resource.path}`;
	}
}

/**
 * A repository's groups as VS Code's Changes view lists them: none until its status is read, and
 * none while nothing changed among several repositories. Otherwise Merge Changes and Staged Changes
 * show only with rows, and Changes always, as Git's groups hide when empty except that one.
 */
function groupItems(repository: ScmTreeRepository, single: boolean): ScmTreeItem[] {
	const { groups } = repository;
	if (groups === null) return [];
	const changed = SCM_GROUP_IDS.some((group) => groups[group].length > 0);
	if (!changed && !single) return [];
	return SCM_GROUP_IDS.filter((group) => group === 'workingTree' || groups[group].length > 0).map(
		(group) => ({
			group,
			kind: 'group',
			repository: repository.repository,
			resources: groups[group],
		}),
	);
}

/**
 * The Changes tree's source over whatever `snapshot` returns when the model asks, so the view
 * refreshes the model after changing it. One visible repository puts its groups at the top, as
 * VS Code does; several give each a row with its groups beneath. Rows below a group are a sorted
 * list or a compact folder tree. Every answer is at hand, so the rows change in the task that
 * refreshes them.
 */
export function scmTreeSource(
	snapshot: () => ScmTreeSnapshot,
): VirtualTreeSource<string, ScmTreeItem> {
	const children = (item: ScmTreeItem): readonly ScmTreeItem[] => {
		const { options, repositories } = snapshot();
		switch (item.kind) {
			case 'workspace': {
				const [only] = repositories;
				if (repositories.length === 1 && only !== undefined) return groupItems(only, true);
				return repositories.map(({ repository }) => ({ kind: 'repository', repository }));
			}
			case 'repository': {
				const repository = repositories.find((entry) => entry.repository === item.repository);
				return repository === undefined ? [] : groupItems(repository, false);
			}
			case 'group':
				return options.mode === 'list'
					? listItems(item.repository, item.resources, options)
					: treeItems(item.repository, item.group, item.resources, options);
			case 'folder':
				return item.children;
			case 'resource':
				return [];
		}
	};
	return {
		children,
		hasChildren: (item) => item.kind !== 'resource',
		key: scmTreeItemKey,
	};
}
