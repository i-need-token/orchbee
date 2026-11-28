/**
 * VS Code's activity bar menu, transcribed from Visual Studio Code (Code - OSS) at commit
 * ef5ca45a6e545e4767e0117905ee9d6120b86e11 as a fresh VS Code with no extensions shows it at its
 * default settings, with the activity bar in its default place:
 *
 * - The bar's menu, `CompositeBar.getContextMenuActions` with the activity bar's
 *   `fillContextMenuActions` and `getActivityBarContextMenuActions`: a checkbox for each container
 *   the bar shows, then each group the activity bar adds after a separator.
 * - The items a right-click on a container's icon puts before the bar's menu,
 *   `CompositeActionViewItem.showContextMenu` with `PaneCompositeBar.getContextMenuActionsForComposite`.
 *
 * The groups and items stand in VS Code's order. An item Orchbee offers names the entry of the
 * toggles row's menu that offers it and the Action whose shortcut that entry shows, as VS Code shows
 * the keybinding of the command an item's id names; every other item names the reason it is left out
 * or the slice that adds it.
 *
 * MIT License
 *
 * Copyright (c) 2015 - present Microsoft Corporation
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
 * associated documentation files (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge, publish, distribute,
 * sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all copies or
 * substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
 * NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
 * DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
 * OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

export interface VSCodeActivityBarMenuItem {
	/** A checkbox, checked while its container is pinned. */
	readonly checkbox?: true;
	readonly id: string;
	readonly orchbee:
		| {
				/** The menu entry that offers the item: a view for a checkbox, else an Action. */
				readonly entry: string;
				readonly kind: 'offered';
				/** The Action whose shortcut the entry shows. */
				readonly shortcutOf: string;
		  }
		| { readonly kind: 'excluded'; readonly reason: string }
		| { readonly kind: 'pending'; readonly slice: string };
	/** VS Code's label; the move's depends on the side the Side Bar takes now. */
	readonly title: string | { readonly left: string; readonly right: string };
}

const OTHER_CONTAINER =
	'The File Bar holds Explorer, Search and Source Control; this container has no File Bar view';

/** The bar's menu, group by group: VS Code draws a separator between each two. */
export const VSCODE_ACTIVITY_BAR_MENU: readonly (readonly VSCodeActivityBarMenuItem[])[] = [
	[
		{
			checkbox: true,
			id: 'workbench.view.explorer',
			orchbee: {
				entry: 'explorer',
				kind: 'offered',
				shortcutOf: 'workbench.file-bar.show-explorer',
			},
			title: 'Explorer',
		},
		{
			checkbox: true,
			id: 'workbench.view.search',
			orchbee: { kind: 'pending', slice: 'R1' },
			title: 'Search',
		},
		{
			checkbox: true,
			id: 'workbench.view.scm',
			orchbee: {
				entry: 'source-control',
				kind: 'offered',
				shortcutOf: 'workbench.file-bar.show-source-control',
			},
			title: 'Source Control',
		},
		{
			checkbox: true,
			id: 'workbench.view.debug',
			orchbee: { kind: 'excluded', reason: OTHER_CONTAINER },
			title: 'Run and Debug',
		},
		{
			checkbox: true,
			id: 'workbench.view.extensions',
			orchbee: { kind: 'excluded', reason: OTHER_CONTAINER },
			title: 'Extensions',
		},
	],
	[
		{
			checkbox: true,
			id: 'toggleAccountsVisibility',
			orchbee: {
				kind: 'excluded',
				reason: 'Orchbee has no accounts, so the File Bar has no Accounts button to show',
			},
			title: 'Accounts',
		},
	],
	[
		{
			id: 'workbench.action.activityBar.position',
			orchbee: {
				kind: 'excluded',
				reason:
					'The toggles keep their row at the top of the File Bar, which has no activity bar to place',
			},
			title: 'Activity Bar Position',
		},
		{
			id: 'workbench.action.activityBar.size',
			orchbee: {
				kind: 'excluded',
				reason: 'The toggles keep the 28px size of the File Bar’s row, which has no activity bar',
			},
			title: 'Activity Bar Size',
		},
		{
			id: 'workbench.action.toggleSidebarPosition',
			orchbee: {
				entry: 'workbench.file-bar.toggle-position',
				kind: 'offered',
				shortcutOf: 'workbench.file-bar.toggle-position',
			},
			title: { left: 'Move Primary Side Bar Right', right: 'Move Primary Side Bar Left' },
		},
		{
			id: 'workbench.action.toggleSidebarVisibility',
			orchbee: {
				entry: 'workbench.file-bar.toggle',
				kind: 'offered',
				shortcutOf: 'workbench.file-bar.toggle',
			},
			title: 'Hide Primary Side Bar',
		},
	],
];

const ONE_ROW_MENU =
	'A right-click on a toggle opens the row’s menu, whose checkbox for the view hides or keeps its toggle';

/** What a right-click on a container's icon shows before the bar's menu, in VS Code's order. */
export const VSCODE_ACTIVITY_ICON_MENU_ADDITIONS: readonly VSCodeActivityBarMenuItem[] = [
	{
		id: 'configureKeybinding/workbench.view.explorer',
		orchbee: {
			kind: 'excluded',
			reason: `${ONE_ROW_MENU}; Keyboard Shortcuts changes the keys of each view’s Show Action`,
		},
		title: 'Configure Keybinding',
	},
	{
		id: 'show.toggleCompositePinned',
		orchbee: { kind: 'excluded', reason: ONE_ROW_MENU },
		title: "Hide '{0}'",
	},
	{
		id: 'show.toggleCompositeBadge',
		orchbee: {
			kind: 'excluded',
			reason: 'Source Control’s count badge follows the File Bar’s badge rule, which has no switch',
		},
		title: 'Hide Badge',
	},
	{
		id: 'moveToMenu',
		orchbee: {
			kind: 'excluded',
			reason:
				'Each File Bar view lives only in the File Bar, which has no Panel or second Side Bar',
		},
		title: 'Move To',
	},
];
