import type { ScmGroupId } from './scm-resource';
import type { ScmTreeItem } from './scm-tree';

/** A file row of a repository's group, as a row command names it. */
export interface ScmChangeTarget {
	readonly group: ScmGroupId;
	readonly path: string;
	readonly repository: string;
}

/** What a row's command acts on. */
export type ScmRowTarget =
	| { readonly changes: readonly ScmChangeTarget[]; readonly kind: 'changes' }
	| { readonly group: ScmGroupId; readonly kind: 'group'; readonly repository: string }
	| { readonly kind: 'repositories'; readonly repositories: readonly string[] };

type ScmFileItem = Extract<ScmTreeItem, { readonly kind: 'folder' | 'resource' }>;

function collectChanges(item: ScmFileItem, changes: Map<string, ScmChangeTarget>): void {
	if (item.kind === 'resource') {
		const { group, path } = item.resource;
		changes.set(`${item.repository}\0${group}\0${path}`, {
			group,
			path,
			repository: item.repository,
		});
		return;
	}
	for (const child of item.children) {
		if (child.kind === 'folder' || child.kind === 'resource') collectChanges(child, changes);
	}
}

/**
 * What a Changes row's command acts on, as VS Code's Source Control runs a row's menu and inline
 * buttons: a file or folder row that is selected acts on every selected file and folder, a folder
 * standing for the files below it, and any other row acts on itself alone. `selection` lists the
 * selected rows in sight.
 */
export function scmChangesRowTarget(
	item: ScmTreeItem,
	selected: boolean,
	selection: () => readonly ScmTreeItem[],
): ScmRowTarget | null {
	switch (item.kind) {
		case 'workspace':
			return null;
		case 'repository': {
			const repositories = selected
				? selection().flatMap((entry) => (entry.kind === 'repository' ? [entry.repository] : []))
				: [item.repository];
			return { kind: 'repositories', repositories };
		}
		case 'group':
			return { group: item.group, kind: 'group', repository: item.repository };
		case 'folder':
		case 'resource': {
			const items = selected
				? selection().filter(
						(entry): entry is ScmFileItem => entry.kind === 'folder' || entry.kind === 'resource',
					)
				: [item];
			const changes = new Map<string, ScmChangeTarget>();
			for (const entry of items) collectChanges(entry, changes);
			return { changes: [...changes.values()], kind: 'changes' };
		}
	}
}

/** What a Repositories row's command acts on: every selected repository when it is selected. */
export function scmRepositoriesRowTarget(
	repository: string,
	selected: boolean,
	selection: () => readonly string[],
): ScmRowTarget {
	return { kind: 'repositories', repositories: selected ? selection() : [repository] };
}
