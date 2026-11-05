import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchActionShortcutLabel } from '../useWorkbenchActionShortcutLabel';
import { resolveWorkbenchConfiguredCommandKeys } from '../../model/workbench-configured-command-keys';
import {
	defaultWorkbenchShortcutBindings,
	type WorkbenchShortcutBindings,
} from '../../utils/workbench-shortcuts';

const NO_CONFIGURED_COMMAND_KEYS = resolveWorkbenchConfiguredCommandKeys([], [], 'apple');

function ShortcutLabelsProbe({
	bindings,
}: {
	readonly bindings: WorkbenchShortcutBindings;
}): ReactNode {
	const labelForAction = useWorkbenchActionShortcutLabel({
		configuredCommandKeys: NO_CONFIGURED_COMMAND_KEYS,
		shortcutBindings: bindings,
		shortcutPlatform: 'apple',
		terminalActionShortcutLabels: { 'terminal.clipboard.paste': '⇧⌘V' },
	});
	return [
		labelForAction('workbench.terminal.create'),
		labelForAction('terminal.clipboard.paste'),
		labelForAction('test.unknown') ?? 'unbound',
	].join('|');
}

describe('useWorkbenchActionShortcutLabel', () => {
	it('projects current Workbench bindings before terminal-owned labels', () => {
		const bindings: WorkbenchShortcutBindings = {
			...defaultWorkbenchShortcutBindings('apple'),
			'workbench.terminal.create': { code: 'KeyN', modifiers: ['mod'] },
		};

		expect(renderToStaticMarkup(createElement(ShortcutLabelsProbe, { bindings }))).toBe(
			'⌘N|⇧⌘V|unbound',
		);
	});
});
