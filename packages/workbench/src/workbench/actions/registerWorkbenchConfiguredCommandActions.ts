import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { workbenchConfiguredCommandTitle } from '../model/workbench-configured-command-name';
import type {
	WorkbenchConfiguredCommand,
	WorkbenchConfiguredCommandOutcome,
	WorkbenchConfiguredCommands,
} from '../types/workbench-configured-commands';

interface RegisterWorkbenchConfiguredCommandActionsOptions {
	readonly commands: WorkbenchConfiguredCommands | undefined;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

/** An Action identity per configured command; the command's own identity changes with its definition. */
const ACTION_ID_PREFIX = 'workbench.configured-command.run:';

/**
 * Offers each configured command as an Action of its own, so Search finds it by name, and follows the
 * list as the configuration changes. The command decides its own refusals, such as one already running.
 */
export function registerWorkbenchConfiguredCommandActions({
	commands,
	runtime,
	scope,
}: RegisterWorkbenchConfiguredCommandActionsOptions): ActionDisposable {
	if (commands === undefined) return { dispose: () => undefined };
	const category = {
		id: 'configured-commands',
		title: workbenchMessages.configuredCommandsCategory,
	};
	let registered: readonly WorkbenchConfiguredCommand[] | null = null;
	let registrations: readonly ActionDisposable[] = [];
	const register = (): void => {
		const catalog = commands.getSnapshot();
		if (catalog === registered) return;
		registered = catalog;
		for (const registration of registrations) registration.dispose();
		registrations = catalog.map((command, index) =>
			runtime.register(
				scope,
				defineAction({
					category,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'workbench-configured-command',
						mode: 'foreground',
					},
					getAvailability: () => ({ kind: 'enabled' }),
					handler: {
						execute: async () => actionResult(await commands.run(command.id)),
					},
					id: `${ACTION_ID_PREFIX}${command.id}`,
					input: noActionInput,
					title: workbenchConfiguredCommandTitle(command, index),
				}),
			),
		);
	};
	register();
	const stop = commands.subscribe(register);
	return {
		dispose: () => {
			stop();
			for (const registration of registrations) registration.dispose();
			registrations = [];
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
