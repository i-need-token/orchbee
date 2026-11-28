import { ActionRuntime, createTypedActionRuntime, type ActionResult } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import {
	DEFAULT_FILE_BAR_LAYOUT,
	hideFileBarToggle,
	keepFileBarToggle,
	shownFileBarToggles,
	type FileBarLayout,
	type FileBarPosition,
	type FileBarRowMenuEntry,
	type FileBarViewId,
} from '../../../file-bar';
import { registerWorkbenchFileBarActions } from '../../actions/registerWorkbenchFileBarActions';
import type { WorkbenchActionCatalog } from '../../actions/workbench-action-ids';
import { buildFileBarRowMenu } from '../file-bar-row-menu';
import {
	VSCODE_ACTIVITY_BAR_MENU,
	VSCODE_ACTIVITY_ICON_MENU_ADDITIONS,
	type VSCodeActivityBarMenuItem,
} from './vscode-activity-bar-menu';

/** One item as the menu shows it. */
interface ShownItem {
	readonly checked: boolean | null;
	readonly disabled: boolean;
	readonly id: string;
	readonly label: string;
	readonly shortcut: string | undefined;
}

/** A stand-in for each Action's shortcut that says which Action it belongs to. */
const shortcutOf = (actionId: string): string => `<${actionId}>`;

/** The groups of VS Code's bar menu that Orchbee offers, as VS Code shows them for a layout. */
function expectedGroups(layout: FileBarLayout, position: FileBarPosition): ShownItem[][] {
	const pinned = ({ orchbee }: VSCodeActivityBarMenuItem): boolean =>
		orchbee.kind === 'offered' && !layout.hiddenToggles.some((view) => view === orchbee.entry);
	const offeredCheckboxes = VSCODE_ACTIVITY_BAR_MENU.flat().filter(
		(item) => item.checkbox === true && item.orchbee.kind === 'offered',
	);
	const pinnedCount = offeredCheckboxes.filter(pinned).length;
	return VSCODE_ACTIVITY_BAR_MENU.map((group) =>
		group.flatMap((item): ShownItem[] => {
			if (item.orchbee.kind !== 'offered') return [];
			// VS Code's checkbox is checked while pinned and greyed for the last container pinned.
			const checked = item.checkbox === true ? pinned(item) : null;
			const title = typeof item.title === 'string' ? item.title : item.title[position];
			return [
				{
					checked,
					disabled: checked === true && pinnedCount === 1,
					id: item.orchbee.entry,
					// Copy says "File Bar" wherever VS Code says "Primary Side Bar".
					label: title.replace('Primary Side Bar', 'File Bar'),
					shortcut: shortcutOf(item.orchbee.shortcutOf),
				},
			];
		}),
	).filter((group) => group.length > 0);
}

function shownGroups(entries: readonly FileBarRowMenuEntry[]): ShownItem[][] {
	const groups: ShownItem[][] = [[]];
	for (const entry of entries) {
		if (entry.kind === 'separator') {
			groups.push([]);
			continue;
		}
		groups.at(-1)?.push({
			checked: entry.kind === 'toggle' ? entry.checked : null,
			disabled: entry.disabled,
			id: entry.id,
			label: entry.label,
			shortcut: entry.shortcut,
		});
	}
	return groups.filter((group) => group.length > 0);
}

/** The File Bar's Actions on one side, and its toggles row's menu for any layout. */
function createHarness(position: FileBarPosition) {
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	const typedRuntime = createTypedActionRuntime<WorkbenchActionCatalog>(runtime);
	const move = vi.fn<(target: FileBarPosition) => Promise<ActionResult<void>>>(() =>
		Promise.resolve({ kind: 'completed' }),
	);
	const toggle = vi.fn<() => void>();
	registerWorkbenchFileBarActions({
		isVisible: true,
		move,
		position,
		scope,
		showView: () => undefined,
		toggle,
		typedRuntime,
	});
	const runs: Promise<unknown>[] = [];
	const setToggleShown = vi.fn<(view: FileBarViewId, shown: boolean) => void>();
	const menu = (layout: FileBarLayout) =>
		buildFileBarRowMenu({
			// The English labels, which are VS Code's titles in File Bar wording.
			formatMessage: (descriptor) =>
				typeof descriptor.defaultMessage === 'string' ? descriptor.defaultMessage : '',
			layout,
			position,
			report: (_source, start) => {
				runs.push(start().completion);
			},
			scope,
			setToggleShown,
			shortcutLabelForAction: shortcutOf,
			typedRuntime,
		});
	return { menu, move, runs, setToggleShown, toggle };
}

const SHOWN: FileBarLayout = { ...DEFAULT_FILE_BAR_LAYOUT, visible: true };

describe('the File Bar toggles row menu', () => {
	it.each<FileBarPosition>(['right', 'left'])(
		'shows VS Code’s activity bar menu in File Bar wording with the File Bar on the %s',
		(position) => {
			const { menu } = createHarness(position);
			for (const layout of [
				SHOWN,
				{ ...SHOWN, hiddenToggles: ['explorer'] },
				{ ...SHOWN, activeView: 'source-control', hiddenToggles: ['source-control'] },
			] satisfies FileBarLayout[]) {
				expect(shownGroups(menu(layout))).toEqual(expectedGroups(layout, position));
			}
		},
	);

	it('leaves out each item VS Code adds for an icon, each with its reason', () => {
		for (const item of VSCODE_ACTIVITY_ICON_MENU_ADDITIONS) {
			expect(item.orchbee).toEqual({ kind: 'excluded', reason: expect.any(String) });
		}
	});

	it('runs each item: a checkbox hides or keeps its toggle, the others their Actions', async () => {
		const { menu, move, runs, setToggleShown, toggle } = createHarness('right');
		for (const layout of [
			SHOWN,
			{ ...SHOWN, hiddenToggles: ['source-control'] },
		] satisfies FileBarLayout[]) {
			for (const entry of menu(layout)) {
				if (entry.kind === 'toggle' && !entry.disabled) entry.run();
			}
		}
		for (const entry of menu(SHOWN)) {
			if (entry.kind === 'command') entry.run();
		}
		await Promise.all(runs);

		expect(setToggleShown.mock.calls).toEqual([
			['explorer', false],
			['source-control', false],
			['source-control', true],
		]);
		expect(move.mock.calls).toEqual([['left']]);
		expect(toggle).toHaveBeenCalledTimes(1);
	});

	it('hides and keeps toggles as VS Code unpins and pins containers', () => {
		const sourceControlActive: FileBarLayout = { ...SHOWN, activeView: 'source-control' };
		// Unpinning the active container opens the default one, or the next pinned without it.
		expect(hideFileBarToggle(sourceControlActive, 'source-control')).toMatchObject({
			activeView: 'explorer',
			hiddenToggles: ['source-control'],
		});
		expect(hideFileBarToggle(SHOWN, 'explorer')).toMatchObject({
			activeView: 'source-control',
			hiddenToggles: ['explorer'],
		});
		// The last pinned container stays, and an unpinned one shows while it is active.
		const explorerHidden = hideFileBarToggle(sourceControlActive, 'explorer');
		expect(hideFileBarToggle(explorerHidden, 'source-control')).toBe(explorerHidden);
		expect(shownFileBarToggles(explorerHidden)).toEqual(['source-control']);
		expect(shownFileBarToggles({ ...explorerHidden, activeView: 'explorer' })).toEqual([
			'explorer',
			'source-control',
		]);
		// Pinning a container opens it.
		expect(keepFileBarToggle(explorerHidden, 'explorer')).toMatchObject({
			activeView: 'explorer',
			hiddenToggles: [],
		});
	});
});
