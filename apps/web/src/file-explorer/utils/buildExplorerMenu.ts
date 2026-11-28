import type { ActionScope, TypedActionRuntime } from '@orchbee/actions';
import type { MessageDescriptor } from '@orchbee/i18n';

import type { FileExplorerActionCatalog } from '../../file-explorer-actions';
import {
	EXPLORER_MENU_COMMANDS,
	explorerMenu,
	type ExplorerMenuTarget,
} from '../model/explorer-menus';
import type {
	ExplorerMenuGroup,
	ExplorerMenuItem,
	FileExplorerReportActionOutcome,
} from '../types/explorer-menu';

interface ExplorerMenuOptions {
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
	readonly report: FileExplorerReportActionOutcome;
	/** The Explorer's own scope, where its Actions resolve. */
	readonly scope: ActionScope;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly target: ExplorerMenuTarget;
	/** The adapter the Explorer's Actions register through, which alone runs them. */
	readonly typedRuntime: TypedActionRuntime<FileExplorerActionCatalog>;
}

/**
 * The Explorer's context menu as it opens: each command of the target's menu whose Action is
 * offered, greyed while the Action is disabled, with the keys that run it, as VS Code's menu shows
 * a command's keybinding. A group whose commands are all withheld leaves no empty group behind.
 */
export function buildExplorerMenu({
	formatMessage,
	report,
	scope,
	shortcutLabelForAction,
	target,
	typedRuntime,
}: ExplorerMenuOptions): readonly ExplorerMenuGroup[] {
	const invocation = { scope, source: 'context-menu' } as const;
	return explorerMenu(target).flatMap((group) => {
		const items = group.flatMap((command): ExplorerMenuItem[] => {
			const { actionId, title } = EXPLORER_MENU_COMMANDS[command];
			const resolved = typedRuntime.resolveAction(actionId, undefined, invocation);
			if (resolved === null || resolved.availability.kind === 'hidden') return [];
			return [
				{
					disabled: resolved.availability.kind === 'disabled',
					id: command,
					label: formatMessage(title),
					run: () =>
						report(invocation.source, () => typedRuntime.execute(actionId, undefined, invocation)),
					shortcut: shortcutLabelForAction(actionId),
				},
			];
		});
		return items.length === 0 ? [] : [items];
	});
}
