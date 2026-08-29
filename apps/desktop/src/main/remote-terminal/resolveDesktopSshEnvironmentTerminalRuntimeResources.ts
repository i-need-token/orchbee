import path from 'node:path';

import {
	TerminalLaunchSchema,
	type TerminalConfiguration,
	type TerminalLaunch,
} from '@orchbee/terminal-protocol';

import { EnvironmentIdSchema, type EnvironmentId } from '../environment';
import { createDefaultTerminalProfile } from '../host';

interface DesktopSshEnvironmentTerminalRuntimeBaseConfiguration {
	readonly terminalArchiveDirectory: string;
	readonly terminalBindingStorePath: string;
	readonly terminalConfiguration: TerminalConfiguration;
	readonly terminalExitPublicationLedgerPath: string;
	readonly terminalExitSpillPath: string;
}

interface DesktopSshEnvironmentTerminalRuntimeResources<
	Configuration extends DesktopSshEnvironmentTerminalRuntimeBaseConfiguration,
> {
	readonly configuration: Configuration;
	readonly launch: TerminalLaunch;
}

export function resolveDesktopSshEnvironmentTerminalRuntimeResources<
	Configuration extends DesktopSshEnvironmentTerminalRuntimeBaseConfiguration,
>(
	base: Configuration,
	environmentId: EnvironmentId,
): DesktopSshEnvironmentTerminalRuntimeResources<Configuration> {
	const canonicalId = EnvironmentIdSchema.parse(environmentId);
	const environmentToken = canonicalId.slice('environment:'.length);
	const recoveryRoot = path.dirname(base.terminalBindingStorePath);
	const environmentRecoveryRoot = path.join(recoveryRoot, 'ssh-environments-v1', environmentToken);
	return {
		configuration: {
			...base,
			terminalArchiveDirectory: path.join(environmentRecoveryRoot, 'terminal-archives-v1'),
			terminalBindingStorePath: path.join(environmentRecoveryRoot, 'bindings-v1.json'),
			terminalExitPublicationLedgerPath: path.join(
				environmentRecoveryRoot,
				'terminal-exit-publications-v1.json',
			),
			terminalExitSpillPath: path.join(environmentRecoveryRoot, 'terminal-exits-v1.json'),
		},
		launch: TerminalLaunchSchema.parse({
			arguments: [],
			cellPixelHeight: null,
			cellPixelWidth: null,
			columns: null,
			configuration: base.terminalConfiguration,
			currentDirectory: null,
			environment: [
				{ name: 'COLORTERM', value: 'truecolor' },
				{ name: 'TERM', value: 'xterm-256color' },
				{ name: 'TERM_PROGRAM', value: 'orchbee' },
			],
			executable: '/bin/sh',
			rows: null,
			terminalProfile: createDefaultTerminalProfile(),
		}),
	};
}
