import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionMessageDescriptor,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchConfiguredCommandKeyNote } from '../model/workbench-configured-command-keys';
import { workbenchConfiguredCommandActionId } from './workbench-configured-command-action-id';
import { workbenchConfiguredCommandTitle } from '../model/workbench-configured-command-name';
import type {
	WorkbenchConfiguredCommand,
	WorkbenchConfiguredCommandOutcome,
	WorkbenchConfiguredCommands,
} from '../types/workbench-configured-commands';

interface RegisterWorkbenchConfiguredCommandActionsOptions {
	/** The list as the window last read it; register it again when it changes. */
	readonly catalog: readonly WorkbenchConfiguredCommand[];
	readonly commands: WorkbenchConfiguredCommands;
	/** What each command's row says about its keys. */
	readonly notes: ReadonlyMap<string, WorkbenchConfiguredCommandKeyNote>;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

const KEY_NOTES = {
	took: workbenchMessages.configuredCommandKeyTook,
	'tui-only': workbenchMessages.configuredCommandKeyTuiOnly,
	yielded: workbenchMessages.configuredCommandKeyYielded,
} as const satisfies Record<WorkbenchConfiguredCommandKeyNote['kind'], unknown>;

function keyNoteMessage(note: WorkbenchConfiguredCommandKeyNote): ActionMessageDescriptor {
	return {
		id: KEY_NOTES[note.kind].id,
		values:
			note.kind === 'tui-only'
				? { shortcut: note.shortcut }
				: { action: note.action, shortcut: note.shortcut },
	};
}

/**
 * Offers each configured command as an Action of its own, so Search finds it by name and its row says
 * what its keys do in this window. The command decides its own refusals, such as one already running.
 */
export function registerWorkbenchConfiguredCommandActions({
	catalog,
	commands,
	notes,
	runtime,
	scope,
}: RegisterWorkbenchConfiguredCommandActionsOptions): ActionDisposable {
	const category = {
		id: 'configured-commands',
		title: workbenchMessages.configuredCommandsCategory,
	};
	const registrations = catalog.map((command, index) => {
		const note = notes.get(command.id);
		return runtime.register(
			scope,
			defineAction({
				category,
				...(note === undefined ? {} : { description: keyNoteMessage(note) }),
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'workbench-configured-command',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: async () => actionResult(await commands.run(command.id)),
				},
				id: workbenchConfiguredCommandActionId(command.id),
				input: noActionInput,
				title: workbenchConfiguredCommandTitle(command, index),
			}),
		);
	});
	return {
		dispose: () => {
			for (const registration of registrations) registration.dispose();
		},
	};
}

function actionResult(outcome: WorkbenchConfiguredCommandOutcome): ActionResult {
	switch (outcome.kind) {
		case 'accepted':
			return { kind: 'completed' };
		case 'refused':
			return { code: outcome.reason, kind: 'blocked' };
		case 'failed':
		case 'unknown':
			return { failure: { code: outcome.kind }, kind: 'failed' };
	}
}
