import type { ActionMessageDescriptor } from '@orchbee/actions';
import { workbenchMessages, type IntlShape } from '@orchbee/i18n';

import type { WorkbenchConfiguredCommand } from '../types/workbench-configured-commands';

/**
 * How Search names a configured command: by its description, else by the text the TUI's key help
 * shows for it, else by its place in the list, since a command's identity is opaque. Neither text
 * carries a message of its own, so Search matches it as written in every language.
 */
export function workbenchConfiguredCommandTitle(
	command: WorkbenchConfiguredCommand,
	index: number,
): ActionMessageDescriptor {
	const name = command.description ?? command.display;
	return name === null
		? { ...workbenchMessages.configuredCommandUnnamed, values: { number: index + 1 } }
		: { id: workbenchMessages.configuredCommandTitle.id, values: { command: name } };
}

/** The same name, as a notice says it; a command the list no longer holds keeps its identity. */
export function formatWorkbenchConfiguredCommandName(
	intl: IntlShape,
	catalog: readonly WorkbenchConfiguredCommand[],
	commandId: string,
): string {
	const index = catalog.findIndex(({ id }) => id === commandId);
	const command = catalog[index];
	if (command === undefined) return commandId;
	return (
		command.description ??
		command.display ??
		intl.formatMessage(workbenchMessages.configuredCommandUnnamed, { number: index + 1 })
	);
}
