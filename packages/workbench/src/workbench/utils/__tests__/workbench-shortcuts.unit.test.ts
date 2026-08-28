import { keyboardShortcutChordKey } from '@orchbee/keyboard-shortcuts';
import { describe, expect, it } from 'vitest';

import {
	formatWorkbenchActionShortcut,
	formatWorkbenchShortcut,
	defaultWorkbenchShortcutBindings,
	resolveWorkbenchShortcut,
	WORKBENCH_SHORTCUTS,
} from '../workbench-shortcuts';

interface ShortcutEventOverrides {
	readonly altKey?: boolean;
	readonly code?: string;
	readonly ctrlKey?: boolean;
	readonly isComposing?: boolean;
	readonly metaKey?: boolean;
	readonly repeat?: boolean;
	readonly shiftKey?: boolean;
}

function shortcutEvent(overrides: ShortcutEventOverrides = {}) {
	return {
		altKey: false,
		code: 'KeyB',
		ctrlKey: false,
		isComposing: false,
		metaKey: false,
		repeat: false,
		shiftKey: false,
		...overrides,
	};
}

describe('Workbench shortcuts', () => {
	it('owns stable semantic actions and platform displays', () => {
		expect(WORKBENCH_SHORTCUTS.actionsSearch.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.closeActiveTerminal.id).toBe('workbench.terminal.active.close');
		expect(WORKBENCH_SHORTCUTS.decreaseTerminalFontSize.id).toBe(
			'workbench.terminal.font-size.decrease',
		);
		expect(WORKBENCH_SHORTCUTS.equalizePanels.id).toBe('workbench.panel.active.equalize');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelDown.id).toBe('workbench.panel.active.focus-down');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelLeft.id).toBe('workbench.panel.active.focus-left');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelNext.id).toBe('workbench.panel.active.focus-next');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelPrevious.id).toBe(
			'workbench.panel.active.focus-previous',
		);
		expect(WORKBENCH_SHORTCUTS.focusActivePanelRight.id).toBe('workbench.panel.active.focus-right');
		expect(WORKBENCH_SHORTCUTS.focusActivePanelUp.id).toBe('workbench.panel.active.focus-up');
		expect(WORKBENCH_SHORTCUTS.increaseTerminalFontSize.id).toBe(
			'workbench.terminal.font-size.increase',
		);
		expect(WORKBENCH_SHORTCUTS.maximizeActivePanel.id).toBe(
			'workbench.panel.active.maximization.toggle',
		);
		expect(WORKBENCH_SHORTCUTS.moveActivePanelDown.id).toBe('workbench.panel.active.move-down');
		expect(WORKBENCH_SHORTCUTS.moveActivePanelLeft.id).toBe('workbench.panel.active.move-left');
		expect(WORKBENCH_SHORTCUTS.moveActivePanelRight.id).toBe('workbench.panel.active.move-right');
		expect(WORKBENCH_SHORTCUTS.moveActivePanelUp.id).toBe('workbench.panel.active.move-up');
		expect(WORKBENCH_SHORTCUTS.newTerminal.id).toBe('workbench.terminal.create');
		expect(WORKBENCH_SHORTCUTS.newTerminalInCurrentDirectory).toMatchObject({
			id: 'workbench.terminal.create-current-directory',
			input: undefined,
			shortcutId: 'workbench.terminal.create-current-directory',
		});
		expect(WORKBENCH_SHORTCUTS.search.id).toBe('workbench.search.open');
		expect(WORKBENCH_SHORTCUTS.moveActiveTabBackward).toMatchObject({
			id: 'workbench.tab.active.move-backward',
			input: undefined,
			shortcutId: 'workbench.tab.active.move-backward',
		});
		expect(WORKBENCH_SHORTCUTS.moveActiveTabForward).toMatchObject({
			id: 'workbench.tab.active.move-forward',
			input: undefined,
			shortcutId: 'workbench.tab.active.move-forward',
		});
		expect(WORKBENCH_SHORTCUTS.setTabTitle).toMatchObject({
			id: 'workbench.tab.title.set',
			input: undefined,
			shortcutId: 'workbench.tab.title.set',
		});
		expect(WORKBENCH_SHORTCUTS.resetTerminalFontSize.id).toBe('workbench.terminal.font-size.reset');
		expect(WORKBENCH_SHORTCUTS.resizeActivePanel.id).toBe('workbench.panel.active.resize');
		expect(WORKBENCH_SHORTCUTS.selectNextTab).toMatchObject({
			id: 'workbench.tab.select-next',
			input: undefined,
			shortcutId: 'workbench.tab.select-next',
		});
		expect(WORKBENCH_SHORTCUTS.selectPreviousTab).toMatchObject({
			id: 'workbench.tab.select-previous',
			input: undefined,
			shortcutId: 'workbench.tab.select-previous',
		});
		expect(WORKBENCH_SHORTCUTS.selectTab1).toMatchObject({
			id: 'workbench.tab.select-by-position',
			input: { position: 1 },
			shortcutId: 'workbench.tab.select.1',
		});
		expect(WORKBENCH_SHORTCUTS.selectTab9).toMatchObject({
			id: 'workbench.tab.select-by-position',
			input: { position: 9 },
			shortcutId: 'workbench.tab.select.9',
		});
		expect(WORKBENCH_SHORTCUTS.settings.id).toBe('workbench.settings.open');
		expect(WORKBENCH_SHORTCUTS.splitActivePanelBelow.id).toBe('workbench.panel.active.split-below');
		expect(WORKBENCH_SHORTCUTS.splitActivePanelRight.id).toBe('workbench.panel.active.split-right');
		expect(WORKBENCH_SHORTCUTS.toggleSidebar.id).toBe('workbench.sidebar.toggle');
		expect(formatWorkbenchShortcut('actionsSearch', 'apple')).toBe('⇧⌘P');
		expect(formatWorkbenchShortcut('closeActiveTerminal', 'apple')).toBe('⌘W');
		expect(formatWorkbenchShortcut('decreaseTerminalFontSize', 'apple')).toBe('⌘-');
		expect(formatWorkbenchShortcut('equalizePanels', 'apple')).toBe('⌃⌘=');
		expect(formatWorkbenchShortcut('focusActivePanelDown', 'apple')).toBe('⌥⌘↓');
		expect(formatWorkbenchShortcut('focusActivePanelLeft', 'apple')).toBe('⌥⌘←');
		expect(formatWorkbenchShortcut('focusActivePanelNext', 'apple')).toBe('⌃⇧]');
		expect(formatWorkbenchShortcut('focusActivePanelPrevious', 'apple')).toBe('⌃⇧[');
		expect(formatWorkbenchShortcut('focusActivePanelRight', 'apple')).toBe('⌥⌘→');
		expect(formatWorkbenchShortcut('focusActivePanelUp', 'apple')).toBe('⌥⌘↑');
		expect(formatWorkbenchShortcut('increaseTerminalFontSize', 'apple')).toBe('⌘=');
		expect(formatWorkbenchShortcut('maximizeActivePanel', 'apple')).toBe('⇧⌘Enter');
		expect(formatWorkbenchShortcut('moveActivePanelDown', 'apple')).toBe('⌥⇧⌘↓');
		expect(formatWorkbenchShortcut('moveActivePanelLeft', 'apple')).toBe('⌥⇧⌘←');
		expect(formatWorkbenchShortcut('moveActivePanelRight', 'apple')).toBe('⌥⇧⌘→');
		expect(formatWorkbenchShortcut('moveActivePanelUp', 'apple')).toBe('⌥⇧⌘↑');
		expect(formatWorkbenchShortcut('moveActiveTabBackward', 'apple')).toBe('⌃⇧,');
		expect(formatWorkbenchShortcut('moveActiveTabForward', 'apple')).toBe('⌃⇧.');
		expect(formatWorkbenchShortcut('setTabTitle', 'apple')).toBe('⇧⌘I');
		expect(formatWorkbenchShortcut('newTerminal', 'apple')).toBe('⌘T');
		expect(formatWorkbenchShortcut('newTerminalInCurrentDirectory', 'apple')).toBe('⌥⌘Enter');
		expect(formatWorkbenchShortcut('search', 'apple')).toBe('⇧⌘F');
		expect(formatWorkbenchShortcut('resetTerminalFontSize', 'apple')).toBe('⌘0');
		expect(formatWorkbenchShortcut('resizeActivePanel', 'apple')).toBe('⌘R');
		expect(formatWorkbenchShortcut('resizeActivePanel', 'standard')).toBe('Ctrl+Shift+R');
		expect(formatWorkbenchShortcut('selectNextTab', 'apple')).toBe('⇧⌘]');
		expect(formatWorkbenchShortcut('selectPreviousTab', 'apple')).toBe('⇧⌘[');
		expect(formatWorkbenchShortcut('selectTab1', 'apple')).toBe('⌘1');
		expect(formatWorkbenchShortcut('selectTab9', 'apple')).toBe('⌘9');
		expect(formatWorkbenchShortcut('settings', 'apple')).toBe('⌘,');
		expect(formatWorkbenchShortcut('splitActivePanelBelow', 'apple')).toBe('⇧⌘D');
		expect(formatWorkbenchShortcut('splitActivePanelRight', 'apple')).toBe('⌘D');
		expect(formatWorkbenchShortcut('toggleSidebar', 'apple')).toBe('⌘B');
		expect(formatWorkbenchShortcut('actionsSearch', 'standard')).toBe('Ctrl+Shift+P');
		expect(formatWorkbenchShortcut('closeActiveTerminal', 'standard')).toBe('Ctrl+Shift+W');
		expect(formatWorkbenchShortcut('decreaseTerminalFontSize', 'standard')).toBe('Ctrl+-');
		expect(formatWorkbenchShortcut('equalizePanels', 'standard')).toBe('Ctrl+Alt+=');
		expect(formatWorkbenchShortcut('focusActivePanelDown', 'standard')).toBe('Ctrl+Alt+↓');
		expect(formatWorkbenchShortcut('focusActivePanelLeft', 'standard')).toBe('Ctrl+Alt+←');
		expect(formatWorkbenchShortcut('focusActivePanelNext', 'standard')).toBe('Ctrl+Shift+]');
		expect(formatWorkbenchShortcut('focusActivePanelPrevious', 'standard')).toBe('Ctrl+Shift+[');
		expect(formatWorkbenchShortcut('focusActivePanelRight', 'standard')).toBe('Ctrl+Alt+→');
		expect(formatWorkbenchShortcut('focusActivePanelUp', 'standard')).toBe('Ctrl+Alt+↑');
		expect(formatWorkbenchShortcut('increaseTerminalFontSize', 'standard')).toBe('Ctrl+=');
		expect(formatWorkbenchShortcut('maximizeActivePanel', 'standard')).toBe('Ctrl+Shift+Enter');
		expect(formatWorkbenchShortcut('moveActivePanelDown', 'standard')).toBe('Ctrl+Alt+Shift+↓');
		expect(formatWorkbenchShortcut('moveActivePanelLeft', 'standard')).toBe('Ctrl+Alt+Shift+←');
		expect(formatWorkbenchShortcut('moveActivePanelRight', 'standard')).toBe('Ctrl+Alt+Shift+→');
		expect(formatWorkbenchShortcut('moveActivePanelUp', 'standard')).toBe('Ctrl+Alt+Shift+↑');
		expect(formatWorkbenchShortcut('moveActiveTabBackward', 'standard')).toBe('Ctrl+Shift+,');
		expect(formatWorkbenchShortcut('moveActiveTabForward', 'standard')).toBe('Ctrl+Shift+.');
		expect(formatWorkbenchShortcut('setTabTitle', 'standard')).toBe('Ctrl+Alt+Shift+T');
		expect(formatWorkbenchShortcut('newTerminal', 'standard')).toBe('Ctrl+Shift+T');
		expect(formatWorkbenchShortcut('newTerminalInCurrentDirectory', 'standard')).toBe(
			'Ctrl+Alt+Enter',
		);
		expect(formatWorkbenchShortcut('search', 'standard')).toBe('Ctrl+Shift+K');
		expect(formatWorkbenchShortcut('resetTerminalFontSize', 'standard')).toBe('Ctrl+0');
		expect(formatWorkbenchShortcut('selectNextTab', 'standard')).toBe('Ctrl+Page Down');
		expect(formatWorkbenchShortcut('selectPreviousTab', 'standard')).toBe('Ctrl+Page Up');
		expect(formatWorkbenchShortcut('selectTab1', 'standard')).toBe('Ctrl+1');
		expect(formatWorkbenchShortcut('selectTab9', 'standard')).toBe('Ctrl+9');
		expect(formatWorkbenchShortcut('settings', 'standard')).toBe('Ctrl+,');
		expect(formatWorkbenchShortcut('splitActivePanelBelow', 'standard')).toBe('Ctrl+Shift+E');
		expect(formatWorkbenchShortcut('splitActivePanelRight', 'standard')).toBe('Ctrl+Shift+D');
		expect(formatWorkbenchShortcut('toggleSidebar', 'standard')).toBe('Ctrl+Shift+B');
		expect(formatWorkbenchActionShortcut('workbench.search.open', 'apple')).toBe('⇧⌘F');
		expect(formatWorkbenchActionShortcut('workbench.panel.active.equalize', 'apple')).toBe('⌃⌘=');
		expect(formatWorkbenchActionShortcut('workbench.panel.active.focus-next', 'apple')).toBe('⌃⇧]');
		expect(formatWorkbenchActionShortcut('workbench.panel.active.focus-previous', 'apple')).toBe(
			'⌃⇧[',
		);
		expect(formatWorkbenchActionShortcut('workbench.panel.active.move-left', 'apple')).toBe('⌥⇧⌘←');
		expect(formatWorkbenchActionShortcut('workbench.tab.active.move-backward', 'apple')).toBe(
			'⌃⇧,',
		);
		expect(formatWorkbenchActionShortcut('workbench.tab.active.move-forward', 'apple')).toBe('⌃⇧.');
		expect(formatWorkbenchActionShortcut('workbench.tab.title.set', 'apple')).toBe('⇧⌘I');
		expect(
			formatWorkbenchActionShortcut('workbench.terminal.create-current-directory', 'apple'),
		).toBe('⌥⌘Enter');
		expect(formatWorkbenchActionShortcut('workbench.terminal.font-size.increase', 'apple')).toBe(
			'⌘=',
		);
		expect(formatWorkbenchActionShortcut('workbench.tab.select-next', 'apple')).toBe('⇧⌘]');
		expect(formatWorkbenchActionShortcut('workbench.tab.select-previous', 'apple')).toBe('⇧⌘[');
		expect(formatWorkbenchActionShortcut('unknown.action', 'apple')).toBe(null);
		expect(
			new Set(
				Object.values(WORKBENCH_SHORTCUTS).map((definition) =>
					keyboardShortcutChordKey(definition.defaults.apple, 'apple'),
				),
			).size,
		).toBe(Object.keys(WORKBENCH_SHORTCUTS).length);
	});

	it.each([
		['actionsSearch', 'KeyP', 'KeyP'],
		['closeActiveTerminal', 'KeyW', 'KeyW'],
		['decreaseTerminalFontSize', 'Minus', 'Minus'],
		['equalizePanels', 'Equal', 'Equal'],
		['focusActivePanelDown', 'ArrowDown', 'ArrowDown'],
		['focusActivePanelLeft', 'ArrowLeft', 'ArrowLeft'],
		['focusActivePanelNext', 'BracketRight', 'BracketRight'],
		['focusActivePanelPrevious', 'BracketLeft', 'BracketLeft'],
		['focusActivePanelRight', 'ArrowRight', 'ArrowRight'],
		['focusActivePanelUp', 'ArrowUp', 'ArrowUp'],
		['increaseTerminalFontSize', 'Equal', 'Equal'],
		['maximizeActivePanel', 'Enter', 'Enter'],
		['moveActivePanelDown', 'ArrowDown', 'ArrowDown'],
		['moveActivePanelLeft', 'ArrowLeft', 'ArrowLeft'],
		['moveActivePanelRight', 'ArrowRight', 'ArrowRight'],
		['moveActivePanelUp', 'ArrowUp', 'ArrowUp'],
		['moveActiveTabBackward', 'Comma', 'Comma'],
		['moveActiveTabForward', 'Period', 'Period'],
		['setTabTitle', 'KeyI', 'KeyT'],
		['newTerminal', 'KeyT', 'KeyT'],
		['newTerminalInCurrentDirectory', 'Enter', 'Enter'],
		['search', 'KeyF', 'KeyK'],
		['resetTerminalFontSize', 'Digit0', 'Digit0'],
		['selectNextTab', 'BracketRight', 'PageDown'],
		['selectPreviousTab', 'BracketLeft', 'PageUp'],
		['selectTab1', 'Digit1', 'Digit1'],
		['selectTab2', 'Digit2', 'Digit2'],
		['selectTab3', 'Digit3', 'Digit3'],
		['selectTab4', 'Digit4', 'Digit4'],
		['selectTab5', 'Digit5', 'Digit5'],
		['selectTab6', 'Digit6', 'Digit6'],
		['selectTab7', 'Digit7', 'Digit7'],
		['selectTab8', 'Digit8', 'Digit8'],
		['selectTab9', 'Digit9', 'Digit9'],
		['settings', 'Comma', 'Comma'],
		['splitActivePanelBelow', 'KeyD', 'KeyE'],
		['splitActivePanelRight', 'KeyD', 'KeyD'],
		['toggleSidebar', 'KeyB', 'KeyB'],
	] as const)('resolves the exact platform modifiers for %s', (action, appleCode, standardCode) => {
		const appleModifiers = WORKBENCH_SHORTCUTS[action].defaults.apple.modifiers;
		const standardModifiers = WORKBENCH_SHORTCUTS[action].defaults.standard.modifiers;
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({
					code: appleCode,
					altKey: appleModifiers.includes('alt'),
					ctrlKey: appleModifiers.includes('control'),
					metaKey: appleModifiers.includes('meta') || appleModifiers.includes('mod'),
					shiftKey: appleModifiers.includes('shift'),
				}),
				'apple',
			),
		).toMatchObject({ action, shortcutId: WORKBENCH_SHORTCUTS[action].shortcutId });
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({
					code: standardCode,
					altKey: standardModifiers.includes('alt'),
					ctrlKey: standardModifiers.includes('control') || standardModifiers.includes('mod'),
					metaKey: standardModifiers.includes('meta'),
					shiftKey: standardModifiers.includes('shift'),
				}),
				'standard',
			),
		).toMatchObject({ action, shortcutId: WORKBENCH_SHORTCUTS[action].shortcutId });
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: appleCode, ctrlKey: true }), 'apple'),
		).toBe(null);
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: standardCode, metaKey: true }), 'standard'),
		).toBe(null);
	});

	it('rejects ambiguous, composing, repeated, and unrelated key events', () => {
		for (const event of [
			shortcutEvent({ altKey: true, metaKey: true }),
			shortcutEvent({ ctrlKey: true, metaKey: true }),
			shortcutEvent({ isComposing: true, metaKey: true }),
			shortcutEvent({ metaKey: true, repeat: true }),
			shortcutEvent({ metaKey: true, shiftKey: true }),
			shortcutEvent({ code: 'KeyZ', metaKey: true }),
		]) {
			expect(resolveWorkbenchShortcut(event, 'apple')).toBe(null);
		}
	});

	it('leaves Apple Command+K available to the focused terminal scope', () => {
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyK', metaKey: true }), 'apple'),
		).toBeNull();
	});

	it('resolves the enumerable binding table to stable shortcut identities', () => {
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyT', metaKey: true }), 'apple'),
		).toEqual(
			expect.objectContaining({
				action: 'newTerminal',
				shortcutId: WORKBENCH_SHORTCUTS.newTerminal.shortcutId,
			}),
		);
		expect(
			resolveWorkbenchShortcut(shortcutEvent({ code: 'Comma', ctrlKey: true }), 'standard'),
		).toEqual(
			expect.objectContaining({
				action: 'settings',
				shortcutId: WORKBENCH_SHORTCUTS.settings.shortcutId,
			}),
		);
		expect(resolveWorkbenchShortcut(shortcutEvent({ code: 'KeyZ', metaKey: true }), 'apple')).toBe(
			null,
		);
	});

	it('formats and resolves a caller-provided binding table without changing semantic identities', () => {
		const defaults = defaultWorkbenchShortcutBindings('standard');
		const bindings = Object.freeze({
			...defaults,
			[WORKBENCH_SHORTCUTS.search.shortcutId]: Object.freeze({
				code: 'KeyG',
				modifiers: Object.freeze(['control', 'shift'] as const),
			}),
		});

		expect(formatWorkbenchShortcut('search', 'standard', bindings)).toBe('Ctrl+Shift+G');
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({ code: 'KeyG', ctrlKey: true, shiftKey: true }),
				'standard',
				bindings,
			),
		).toMatchObject({ action: 'search', shortcutId: 'workbench.search.all.open' });
		expect(
			resolveWorkbenchShortcut(
				shortcutEvent({ code: 'KeyK', ctrlKey: true, shiftKey: true }),
				'standard',
				bindings,
			),
		).toBe(null);
	});
});
