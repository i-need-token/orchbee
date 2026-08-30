import path from 'node:path';

import {
	TerminalLaunchSchema,
	type TerminalConfiguration,
	type TerminalLaunch,
} from '@orchbee/terminal-protocol';
import { SshEnvironmentLoginShellSchema } from '@orchbee/ssh-environment-protocol';

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

export function resolveDesktopSshEnvironmentTerminalRuntimeConfiguration<
	Configuration extends DesktopSshEnvironmentTerminalRuntimeBaseConfiguration,
>(base: Configuration, environmentId: EnvironmentId): Configuration {
	const environmentRecoveryRoot = resolveEnvironmentRecoveryRoot(base, environmentId);
	return {
		...base,
		terminalArchiveDirectory: path.join(environmentRecoveryRoot, 'terminal-archives-v1'),
		terminalBindingStorePath: path.join(environmentRecoveryRoot, 'bindings-v1.json'),
		terminalExitPublicationLedgerPath: path.join(
			environmentRecoveryRoot,
			'terminal-exit-publications-v1.json',
		),
		terminalExitSpillPath: path.join(environmentRecoveryRoot, 'terminal-exits-v1.json'),
	};
}

export function resolveDesktopSshEnvironmentTerminalRuntimeResources<
	Configuration extends DesktopSshEnvironmentTerminalRuntimeBaseConfiguration,
>(
	base: Configuration,
	environmentId: EnvironmentId,
	loginShell: string,
): DesktopSshEnvironmentTerminalRuntimeResources<Configuration> {
	const executable = SshEnvironmentLoginShellSchema.parse(loginShell);
	return {
		configuration: resolveDesktopSshEnvironmentTerminalRuntimeConfiguration(base, environmentId),
		launch: TerminalLaunchSchema.parse({
			arguments: ['-l'],
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
			executable,
			rows: null,
			terminalProfile: createDefaultTerminalProfile(),
		}),
	};
}

function resolveEnvironmentRecoveryRoot(
	base: DesktopSshEnvironmentTerminalRuntimeBaseConfiguration,
	environmentId: EnvironmentId,
): string {
	const canonicalId = EnvironmentIdSchema.parse(environmentId);
	const environmentToken = canonicalId.slice('environment:'.length);
	const recoveryRoot = path.dirname(base.terminalBindingStorePath);
	return path.join(recoveryRoot, 'ssh-environments-v1', environmentToken);
}
