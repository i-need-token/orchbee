import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import { FileOverlayRegistry, type FileTabMenuEntry } from '../../../file-overlay';
import { registerWorkbenchFileOverlayActions } from '../../actions/registerWorkbenchFileOverlayActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
} from '../../actions/workbench-action-ids';
import { buildFileTabMenuEntries } from '../buildFileTabMenuEntries';
import {
	VSCODE_EDITOR_TITLE_CONTEXT,
	type VSCodeCondition,
	type VSCodeMenuItem,
} from './vscode-editor-title-context';

const WORKSPACE_ID = 'workspace-1';

/** What VS Code's tab menu knows of the tab it opened on. */
interface TabFacts {
	readonly count: number;
	readonly last: boolean;
	readonly preview: boolean;
	readonly sticky: boolean;
}

/** One item as a menu shows it: its Action, label and whether it is greyed. */
interface ShownItem {
	readonly action: string;
	readonly disabled: boolean;
	readonly label: string;
}

/**
 * VS Code's menu order: groups by locale order with an ungrouped item last, then by order, then by
 * title.
 */
function compareVSCodeMenuItems(first: VSCodeMenuItem, second: VSCodeMenuItem): number {
	if (first.group !== second.group) {
		if (first.group === '') return 1;
		if (second.group === '') return -1;
		const groups = first.group.localeCompare(second.group);
		if (groups !== 0) return groups;
	}
	if (first.order !== second.order) return first.order - second.order;
	return first.title.localeCompare(second.title);
}

/** Evaluates a condition with the context keys VS Code sets for the tab right-clicked. */
function holds(condition: VSCodeCondition | undefined, facts: TabFacts): boolean {
	if (condition === undefined) return true;
	if ('and' in condition) return condition.and.every((part) => holds(part, facts));
	if ('not' in condition) return !holds(condition.not, facts);
	if ('notEquals' in condition) return facts.count !== condition.notEquals;
	switch (condition.key) {
		case 'activeEditorCannotClose':
		case 'multipleEditorsSelectedInGroup':
			return false;
		case 'activeEditorIsLastInGroup':
			return facts.last;
		case 'activeEditorIsNotPreview':
			return !facts.preview;
		case 'activeEditorIsPinned':
			return facts.sticky;
		case 'config.workbench.editor.enablePreview':
		case 'editorTabsVisible':
			return true;
	}
}

/** The items VS Code shows for a tab that Orchbee offers, group by group. */
function expectedGroups(facts: TabFacts): ShownItem[][] {
	const groups = new Map<string, ShownItem[]>();
	for (const item of VSCODE_EDITOR_TITLE_CONTEXT) {
		if (item.orchbee.kind !== 'offered' || !holds(item.when, facts)) continue;
		const shown = {
			action: item.orchbee.action,
			disabled: !holds(item.precondition, facts),
			label: item.title,
		};
		groups.set(item.group, [...(groups.get(item.group) ?? []), shown]);
	}
	return [...groups.values()];
}

function shownGroups(entries: readonly FileTabMenuEntry[]): ShownItem[][] {
	const groups: ShownItem[][] = [[]];
	for (const entry of entries) {
		if (entry.kind === 'separator') groups.push([]);
		else groups.at(-1)?.push({ action: entry.id, disabled: entry.disabled, label: entry.label });
	}
	return groups.filter((group) => group.length > 0);
}

/** The shown Workspace's overlay with its Actions, and the menu of any of its tabs. */
function createHarness() {
	const registry = new FileOverlayRegistry(true);
	const store = registry.store(WORKSPACE_ID);
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const typedRuntime = createTypedActionRuntime<WorkbenchActionCatalog>(runtime);
	registerWorkbenchFileOverlayActions({
		dispatch: (event) => {
			store.dispatch(event);
		},
		getState: () => store.state.getState(),
		scope,
		typedRuntime,
		workspaceId: WORKSPACE_ID,
	});
	const runs: Promise<unknown>[] = [];
	return {
		menu: (tabId: string) =>
			buildFileTabMenuEntries({
				// The English labels, which are VS Code's titles.
				formatMessage: (descriptor) =>
					typeof descriptor.defaultMessage === 'string' ? descriptor.defaultMessage : '',
				report: (start) => {
					runs.push(start().completion);
				},
				scope,
				shortcutLabelForAction: () => undefined,
				tabId,
				typedRuntime,
				workspaceId: WORKSPACE_ID,
			}),
		runs,
		store,
	};
}

describe('buildFileTabMenuEntries', () => {
	it('keeps the VS Code tab menu data in VS Code’s menu order', () => {
		expect([...VSCODE_EDITOR_TITLE_CONTEXT].sort(compareVSCodeMenuItems)).toEqual(
			VSCODE_EDITOR_TITLE_CONTEXT,
		);
	});

	it('shows VS Code’s tab menu for every tab: its items, groups, titles and greyed items', () => {
		const mismatches: string[] = [];
		// Every row of up to three tabs, each opened in preview or not, with any count pinned.
		for (let count = 1; count <= 3; count += 1) {
			for (let previewMask = 0; previewMask < 2 ** count; previewMask += 1) {
				for (let stickyCount = 0; stickyCount <= count; stickyCount += 1) {
					const { menu, store } = createHarness();
					for (let index = 0; index < count; index += 1) {
						const path = `/file-${index}.ts`;
						store.dispatch({
							input: { kind: 'file', resource: { key: path, path } },
							preview: (previewMask & (1 << index)) !== 0,
							type: 'file-opened',
						});
					}
					for (const tab of store.state.getState().tabs.tabs.slice(0, stickyCount)) {
						store.dispatch({ id: tab.id, type: 'tab-stuck' });
					}
					const { tabs } = store.state.getState();
					for (const [index, tab] of tabs.tabs.entries()) {
						const facts = {
							count: tabs.tabs.length,
							last: index === tabs.tabs.length - 1,
							preview: tab.id === tabs.previewId,
							sticky: index < tabs.stickyCount,
						};
						const shown = shownGroups(menu(tab.id));
						const expected = expectedGroups(facts);
						if (JSON.stringify(shown) !== JSON.stringify(expected)) {
							mismatches.push(
								`${JSON.stringify(facts)}: shown ${JSON.stringify(shown)}, VS Code ${JSON.stringify(expected)}`,
							);
						}
					}
				}
			}
		}
		expect(mismatches).toEqual([]);
	});

	it('acts on the tab right-clicked, not the active one', async () => {
		const { menu, runs, store } = createHarness();
		for (const path of ['/a.ts', '/b.ts', '/c.ts']) {
			store.dispatch({
				input: { kind: 'file', resource: { key: path, path } },
				preview: false,
				type: 'file-opened',
			});
		}
		const [first] = store.state.getState().tabs.tabs;
		const closeOthers = menu(first?.id ?? '').find(
			(entry) => entry.id === WORKBENCH_ACTION_IDS.closeOtherFileOverlayTabs,
		);
		if (closeOthers?.kind === 'command') closeOthers.run();
		await Promise.all(runs);
		expect(store.state.getState().tabs.tabs.map((tab) => tab.input.resource.path)).toEqual([
			'/a.ts',
		]);
	});
});
