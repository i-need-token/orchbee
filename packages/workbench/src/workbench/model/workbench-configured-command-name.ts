import type { ActionMessageDescriptor } from '@orchbee/actions';
import { workbenchMessages, type IntlShape } from '@orchbee/i18n';

import type { WorkbenchConfiguredCommand } from '../types/workbench-configured-commands';

/**
 * How Search names a configured command: by its description, else by its place in the list, since a
 * command's identity is opaque. A description carries no message of its own, so Search matches it as
 * written in every language.
 */
export function workbenchConfiguredCommandTitle(
	command: WorkbenchConfiguredCommand,
	index: number,
): ActionMessageDescriptor {
	return command.description === null
		? { ...workbenchMessages.configuredCommandUnnamed, values: { number: index + 1 } }
		: { id: workbenchMessages.configuredCommandTitle.id, values: { command: command.description } };
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
		intl.formatMessage(workbenchMessages.configuredCommandUnnamed, { number: index + 1 })
	);
}
