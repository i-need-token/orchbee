import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import type { FileExplorerActionCatalog } from '../../../file-explorer-actions';
import type { FileOverlayOpenFile } from '../../../file-overlay';
import { MemoryFileSessionPort } from '../../../file-session';
import { registerFileExplorerActions } from '../../actions/registerFileExplorerActions';
import { EXPLORER_MENU_COMMANDS, type ExplorerMenuTarget } from '../../model/explorer-menus';
import { ExplorerTree } from '../../model/ExplorerTree';
import { ExplorerViewMemory } from '../../model/ExplorerViewMemory';
import type { ExplorerMenuGroup } from '../../types/explorer-menu';
import { buildExplorerMenu } from '../buildExplorerMenu';
import {
	VSCODE_EXPLORER_CONTEXT,
	type VSCodeCondition,
	type VSCodeMenuItem,
} from './vscode-explorer-menu';

const WORKSPACE = 'workspace';

/**
 * What VS Code's menu knows of the item it is for. The empty space below the rows stands for the
 * root, which VS Code describes as a folder.
 */
const TARGETS: Readonly<Record<ExplorerMenuTarget, { readonly folder: boolean }>> = {
	file: { folder: false },
	folder: { folder: true },
	root: { folder: true },
};

/** One item as the menu shows it: its Action, label and whether it is greyed. */
interface ShownItem {
	readonly action: string;
	readonly disabled: boolean;
	readonly label: string;
}

/**
 * VS Code's menu order: the `navigation` group first, an ungrouped item last and the other groups by
 * locale order, then by order, then by title.
 */
function compareVSCodeMenuItems(first: VSCodeMenuItem, second: VSCodeMenuItem): number {
	if (first.group !== second.group) {
		if (first.group === '') return 1;
		if (second.group === '') return -1;
		if (first.group === 'navigation') return -1;
		if (second.group === 'navigation') return 1;
		const groups = first.group.localeCompare(second.group);
		if (groups !== 0) return groups;
	}
	if (first.order !== second.order) return first.order - second.order;
	return first.title.localeCompare(second.title);
}

/** Evaluates a condition with the context keys VS Code sets for the item a menu is for. */
function holds(condition: VSCodeCondition | undefined, folder: boolean): boolean {
	if (condition === undefined) return true;
	if ('and' in condition) return condition.and.every((part) => holds(part, folder));
	if ('not' in condition) return !holds(condition.not, folder);
	switch (condition.key) {
		case 'explorerResourceIsFolder':
			return folder;
		// Every item the Explorer shows is a resource of the file system.
		case 'isFileSystemResource':
		case 'resourceSet':
			return true;
	}
}

/** The items Orchbee offers that VS Code's menu shows for a target, group by group. */
function expectedGroups(target: ExplorerMenuTarget): ShownItem[][] {
	const { folder } = TARGETS[target];
	const groups = new Map<string, ShownItem[]>();
	for (const item of VSCODE_EXPLORER_CONTEXT) {
		if (item.orchbee.kind !== 'offered' || !holds(item.when, folder)) continue;
		const shown = {
			action: item.orchbee.action,
			disabled: !holds(item.precondition, folder),
			label: item.title,
		};
		groups.set(item.group, [...(groups.get(item.group) ?? []), shown]);
	}
	return [...groups.values()];
}

function shownGroups(groups: readonly ExplorerMenuGroup[]): ShownItem[][] {
	return groups.map((group) =>
		group.map((item) => ({
			action: EXPLORER_MENU_COMMANDS[item.id].actionId,
			disabled: item.disabled,
			label: item.label,
		})),
	);
}

async function settle(): Promise<void> {
	for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
}

/** An Explorer over a Workspace directory with its Actions, its menus, and what they copy. */
async function createHarness() {
	const port = new MemoryFileSessionPort();
	port.writeFile('/w/a.txt', 'a');
	port.writeFile('/w/deep/a/b/c.txt', 'c');
	port.writeFile('/w/docs/d.md', 'd');
	port.setRoot(WORKSPACE, '/w');
	const session = port.open(WORKSPACE);
	const tree = new ExplorerTree({
		locale: 'en-US',
		memory: new ExplorerViewMemory(null),
		reportUnreadable: () => undefined,
		session,
	});
	await settle();
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('file-explorer');
	const typedRuntime = createTypedActionRuntime<FileExplorerActionCatalog>(runtime);
	const copied: string[] = [];
	const openFile: FileOverlayOpenFile = () => {
		throw new Error('The menu opened a file.');
	};
	registerFileExplorerActions({
		clipboard: {
			lineBreak: '\n',
			writeText: (text) => {
				copied.push(text);
				return Promise.resolve();
			},
		},
		openFile,
		scope,
		session,
		tree,
		typedRuntime,
	});
	const runs: Promise<unknown>[] = [];
	const menu = (target: ExplorerMenuTarget) =>
		buildExplorerMenu({
			// The English labels, which are VS Code's titles.
			formatMessage: (descriptor) =>
				typeof descriptor.defaultMessage === 'string' ? descriptor.defaultMessage : '',
			report: (_source, start) => {
				runs.push(start().completion);
			},
			scope,
			shortcutLabelForAction: () => undefined,
			target,
			typedRuntime,
		});
	const { state } = tree;
	if (state.kind !== 'tree') throw new Error(`The Explorer shows ${state.kind}.`);
	return { copied, menu, model: state.model, runs };
}

describe('Explorer context menu', () => {
	it('keeps the VS Code menu data in VS Code’s menu order', () => {
		expect([...VSCODE_EXPLORER_CONTEXT].sort(compareVSCodeMenuItems)).toEqual(
			VSCODE_EXPLORER_CONTEXT,
		);
	});

	it('shows VS Code’s menu for a file, a folder and the root: its items, groups and titles', async () => {
		const { menu } = await createHarness();
		for (const target of ['file', 'folder', 'root'] as const) {
			expect({ target, groups: shownGroups(menu(target)) }).toEqual({
				target,
				groups: expectedGroups(target),
			});
		}
	});

	// VS Code's Explorer commands act on its `getContext` rule.
	it('copies the paths of what VS Code’s Explorer commands act on, one per line', async () => {
		const { copied, menu, model, runs } = await createHarness();
		await model.expand('/w/deep');
		await settle();
		const cases = [
			// Nothing selected or focused, after a right click below the rows: the root.
			{ focus: null, paths: '/w', relative: '', selection: [] },
			// Several rows: each selected row in row order, a compact row with each folder it joins.
			{
				focus: '/w/a.txt',
				paths: '/w/deep\n/w/deep/a\n/w/deep/a/b\n/w/a.txt',
				relative: 'deep\ndeep/a\ndeep/a/b\na.txt',
				selection: ['/w/a.txt', '/w/deep/a/b'],
			},
			// One compact row with focus on one of its folders: that folder alone.
			{ focus: '/w/deep/a', paths: '/w/deep/a', relative: 'deep/a', selection: ['/w/deep/a/b'] },
			// Focus on a row the selection does not hold: the focused row.
			{ focus: '/w/docs', paths: '/w/docs', relative: 'docs', selection: ['/w/a.txt'] },
		];
		for (const { focus, paths, relative, selection } of cases) {
			model.setSelection(selection);
			model.setFocus(focus);
			copied.length = 0;
			for (const item of menu('folder').flat()) item.run();
			await Promise.all(runs);
			expect({ focus, selection, copied }).toEqual({
				focus,
				selection,
				copied: [paths, relative],
			});
		}
	});
});
