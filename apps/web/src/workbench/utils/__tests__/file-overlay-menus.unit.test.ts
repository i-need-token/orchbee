import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import { FileOverlayRegistry, type FileOverlayMenuEntry } from '../../../file-overlay';
import { registerWorkbenchFileOverlayActions } from '../../actions/registerWorkbenchFileOverlayActions';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
} from '../../actions/workbench-action-ids';
import { buildFileOverlayTitleActions, buildFileTabMenuEntries } from '../file-overlay-menus';
import {
	VSCODE_EDITOR_TITLE,
	VSCODE_EDITOR_TITLE_CONTEXT,
	type VSCodeCondition,
	type VSCodeMenuItem,
} from './vscode-editor-menus';

const WORKSPACE_ID = 'workspace-1';

/** What VS Code's menus know of the tab they are for. */
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

/** Evaluates a condition with the context keys VS Code sets for the tab a menu is for. */
function holds(condition: VSCodeCondition | undefined, facts: TabFacts): boolean {
	if (condition === undefined) return true;
	if ('and' in condition) return condition.and.every((part) => holds(part, facts));
	if ('not' in condition) return !holds(condition.not, facts);
	if ('notEquals' in condition) return facts.count !== condition.notEquals;
	switch (condition.key) {
		case 'activeEditorCannotClose':
		case 'editorPartModal':
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

/** The items of a VS Code menu that Orchbee offers and VS Code shows for a tab, group by group. */
function expectedGroups(menu: readonly VSCodeMenuItem[], facts: TabFacts): ShownItem[][] {
	const groups = new Map<string, ShownItem[]>();
	for (const item of menu) {
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

function shownGroups(entries: readonly FileOverlayMenuEntry[]): ShownItem[][] {
	const groups: ShownItem[][] = [[]];
	for (const entry of entries) {
		if (entry.kind === 'separator') groups.push([]);
		else groups.at(-1)?.push({ action: entry.id, disabled: entry.disabled, label: entry.label });
	}
	return groups.filter((group) => group.length > 0);
}

/** The shown Workspace's overlay with its Actions, and the menus for any of its tabs. */
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
		focusOverlay: () => undefined,
		getState: () => store.state.getState(),
		quickNavigateKeys: () => [],
		scope,
		tabPicker: {
			navigate: () => false,
			pick: () => Promise.resolve(null),
			showing: () => false,
		},
		typedRuntime,
		workspaceId: WORKSPACE_ID,
	});
	const runs: Promise<unknown>[] = [];
	const options = (tabId: string) => ({
		// The English labels, which are VS Code's titles.
		formatMessage: (descriptor: { readonly defaultMessage?: unknown }) =>
			typeof descriptor.defaultMessage === 'string' ? descriptor.defaultMessage : '',
		report: (_source: unknown, start: () => { readonly completion: Promise<unknown> }) => {
			runs.push(start().completion);
		},
		scope,
		shortcutLabelForAction: () => undefined,
		tabId,
		typedRuntime,
		workspaceId: WORKSPACE_ID,
	});
	return {
		menu: (tabId: string) => buildFileTabMenuEntries(options(tabId)),
		runs,
		store,
		titleActions: (tabId: string) => buildFileOverlayTitleActions(options(tabId)),
	};
}

/**
 * Every row of up to three tabs, each opened in preview or not, with any count pinned, and each tab
 * of the row with what VS Code's menus know of it.
 */
function* everyTab(): Generator<{
	readonly facts: TabFacts;
	readonly harness: ReturnType<typeof createHarness>;
	readonly tabId: string;
}> {
	for (let count = 1; count <= 3; count += 1) {
		for (let previewMask = 0; previewMask < 2 ** count; previewMask += 1) {
			for (let stickyCount = 0; stickyCount <= count; stickyCount += 1) {
				const harness = createHarness();
				const { store } = harness;
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
					yield { facts, harness, tabId: tab.id };
				}
			}
		}
	}
}

describe('file overlay menus', () => {
	it('keeps the VS Code menu data in VS Code’s menu order', () => {
		for (const menu of [VSCODE_EDITOR_TITLE_CONTEXT, VSCODE_EDITOR_TITLE]) {
			expect([...menu].sort(compareVSCodeMenuItems)).toEqual(menu);
		}
	});

	it('shows VS Code’s tab menu for every tab: its items, groups, titles and greyed items', () => {
		const mismatches: string[] = [];
		for (const { facts, harness, tabId } of everyTab()) {
			const shown = shownGroups(harness.menu(tabId));
			const expected = expectedGroups(VSCODE_EDITOR_TITLE_CONTEXT, facts);
			if (JSON.stringify(shown) !== JSON.stringify(expected)) {
				mismatches.push(
					`${JSON.stringify(facts)}: shown ${JSON.stringify(shown)}, VS Code ${JSON.stringify(expected)}`,
				);
			}
		}
		expect(mismatches).toEqual([]);
	});

	it('shows VS Code’s title actions for every active tab: buttons, then More Actions by group', () => {
		const mismatches: string[] = [];
		for (const { facts, harness, tabId } of everyTab()) {
			harness.store.dispatch({ id: tabId, type: 'tab-activated' });
			const { buttons, more } = harness.titleActions(tabId);
			const shown = [
				buttons.map((button) => ({
					action: button.id,
					disabled: button.disabled,
					label: button.label,
				})),
				...shownGroups(more),
			];
			const [navigation = []] = expectedGroups(
				VSCODE_EDITOR_TITLE.filter((item) => item.group === 'navigation'),
				facts,
			);
			const expected = [
				navigation,
				...expectedGroups(
					VSCODE_EDITOR_TITLE.filter((item) => item.group !== 'navigation'),
					facts,
				),
			];
			if (JSON.stringify(shown) !== JSON.stringify(expected)) {
				mismatches.push(
					`${JSON.stringify(facts)}: shown ${JSON.stringify(shown)}, VS Code ${JSON.stringify(expected)}`,
				);
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
