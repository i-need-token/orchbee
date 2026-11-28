import type { ScmGroupId } from './scm-resource';
import type { ScmTreeItem } from './scm-tree';
import type { ScmViewMode } from './scm-view-state';

/**
 * A command a Source Control row offers, named after the VS Code command it stands for: `clean` is
 * `git.clean`, Discard Changes, and `collapse-all` the view's own Collapse All.
 */
export type ScmRowCommand =
	| 'clean'
	| 'clean-all'
	| 'close'
	| 'close-others'
	| 'collapse-all'
	| 'ignore'
	| 'open-change'
	| 'open-file'
	| 'open-head-file'
	| 'reveal-in-explorer'
	| 'reveal-in-os'
	| 'stage'
	| 'stage-all'
	| 'unstage'
	| 'unstage-all'
	| 'view-changes'
	| 'view-staged-changes';

/** The row a menu is for. */
export type ScmMenuTarget =
	| { readonly group: ScmGroupId; readonly kind: 'folder' | 'resource' }
	| { readonly group: ScmGroupId; readonly kind: 'group'; readonly mode: ScmViewMode }
	/** A repository's row, among this many shown. */
	| { readonly kind: 'repository'; readonly repositories: number };

export interface ScmRowMenus {
	/** The context menu's groups in order, with a separator between each two. */
	readonly context: readonly (readonly ScmRowCommand[])[];
	/** The buttons the row shows while hovered, focused or selected, left to right. */
	readonly inline: readonly ScmRowCommand[];
}

const NO_MENUS: ScmRowMenus = { context: [], inline: [] };

function groupMenus(group: ScmGroupId, mode: ScmViewMode): ScmRowMenus {
	const collapse: readonly (readonly ScmRowCommand[])[] = mode === 'tree' ? [['collapse-all']] : [];
	if (group === 'index') {
		return {
			context: [['unstage-all'], ...collapse],
			inline: ['view-staged-changes', 'unstage-all'],
		};
	}
	if (group === 'workingTree') {
		return {
			context: [['clean-all', 'stage-all'], ...collapse],
			inline: ['view-changes', 'clean-all', 'stage-all'],
		};
	}
	return NO_MENUS;
}

function folderMenus(group: ScmGroupId): ScmRowMenus {
	if (group === 'index') return { context: [['unstage']], inline: ['unstage'] };
	if (group === 'workingTree') {
		return { context: [['clean', 'stage', 'ignore']], inline: ['clean', 'stage'] };
	}
	return NO_MENUS;
}

function resourceMenus(group: ScmGroupId, local: boolean): ScmRowMenus {
	const open: readonly ScmRowCommand[] = ['open-change', 'open-file', 'open-head-file'];
	const reveal: readonly ScmRowCommand[] = local
		? ['reveal-in-os', 'reveal-in-explorer']
		: ['reveal-in-explorer'];
	if (group === 'index') {
		return { context: [open, ['unstage'], reveal], inline: ['open-file', 'unstage'] };
	}
	if (group === 'workingTree') {
		return {
			context: [open, ['clean', 'stage', 'ignore'], reveal],
			inline: ['open-file', 'clean', 'stage'],
		};
	}
	return NO_MENUS;
}

/**
 * A row's menus as VS Code's Git extension contributes them with its default settings: a click opens
 * the diff, so a file's inline button opens the file, and untracked files mix into Changes. Groups
 * sort by VS Code's rule, `navigation` first and then by name, and commands inside one by order and
 * then by title. Merge Changes waits for its batch, so its rows offer nothing yet. Reveal in File
 * Explorer applies only to this computer's files, as VS Code's applies only outside a remote.
 */
export function scmRowMenus(target: ScmMenuTarget, local: boolean): ScmRowMenus {
	switch (target.kind) {
		case 'repository':
			return {
				context: [target.repositories > 1 ? ['close', 'close-others'] : ['close']],
				inline: [],
			};
		case 'group':
			return groupMenus(target.group, target.mode);
		case 'folder':
			return folderMenus(target.group);
		case 'resource':
			return resourceMenus(target.group, local);
	}
}

/** What a Changes row's menus are for; null for the Workspace's own row, which offers none. */
export function scmItemMenuTarget(
	item: ScmTreeItem,
	mode: ScmViewMode,
	repositories: number,
): ScmMenuTarget | null {
	switch (item.kind) {
		case 'workspace':
			return null;
		case 'repository':
			return { kind: 'repository', repositories };
		case 'group':
			return { group: item.group, kind: 'group', mode };
		case 'folder':
			return { group: item.group, kind: 'folder' };
		case 'resource':
			return { group: item.resource.group, kind: 'resource' };
	}
}
