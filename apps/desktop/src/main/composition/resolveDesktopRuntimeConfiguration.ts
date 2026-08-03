import { createHash } from 'node:crypto';
import path from 'node:path';

import { TerminalLaunchV1_1Schema, type TerminalLaunchV1_1 } from '@orchbee/terminal-protocol';

import { createDefaultTerminalProfile } from '../host';

export interface DesktopRuntimeEnvironment {
	readonly ORCHBEE_DESKTOP_ENDPOINT_ID?: string;
	readonly ORCHBEE_DESKTOP_GATEWAY?: string;
	readonly ORCHBEE_DESKTOP_RECOVERY_DIRECTORY?: string;
	readonly ORCHBEE_DESKTOP_RUNTIME_DIRECTORY?: string;
	readonly ORCHBEE_GATEWAY_ENTRY_PATH?: string;
	readonly ORCHBEE_TERMINAL_HOST_BINARY?: string;
	readonly SHELL?: string;
	readonly ComSpec?: string;
}

export type DesktopGatewayRuntimeConfiguration =
	| { readonly kind: 'disabled' }
	| { readonly entryPath: string; readonly kind: 'enabled' };

export interface DesktopRuntimeConfigurationInput {
	readonly applicationPath: string;
	readonly environment: DesktopRuntimeEnvironment;
	readonly homeDirectory: string;
	readonly platform: NodeJS.Platform;
	readonly userDataPath: string;
}

export interface DesktopRuntimeConfiguration {
	readonly binaryPath: string;
	readonly endpointId: string;
	readonly gateway: DesktopGatewayRuntimeConfiguration;
	readonly runtimeDirectory: string;
	readonly terminalBindingStorePath: string;
	readonly terminalLaunch: TerminalLaunchV1_1;
	readonly workbenchDocumentStorePath: string;
}

export function resolveDesktopRuntimeConfiguration(
	input: DesktopRuntimeConfigurationInput,
): DesktopRuntimeConfiguration {
	const binaryName = input.platform === 'win32' ? 'terminal-host.exe' : 'terminal-host';
	const configuredBinary = input.environment.ORCHBEE_TERMINAL_HOST_BINARY;
	const configuredRuntimeDirectory = input.environment.ORCHBEE_DESKTOP_RUNTIME_DIRECTORY;
	const binaryPath = configuredBinary
		? path.resolve(configuredBinary)
		: path.resolve(input.applicationPath, '..', 'terminal-host', 'zig-out', 'bin', binaryName);
	let runtimeDirectory: string;
	if (configuredRuntimeDirectory !== undefined) {
		runtimeDirectory = path.resolve(configuredRuntimeDirectory);
	} else if (input.platform === 'win32') {
		runtimeDirectory = path.join(input.userDataPath, 'terminal-host');
	} else {
		runtimeDirectory = path.join(
			'/tmp',
			`orchbee-desktop-${createHash('sha256').update(input.homeDirectory).digest('hex').slice(0, 12)}`,
		);
	}
	const endpointId =
		input.environment.ORCHBEE_DESKTOP_ENDPOINT_ID ??
		createHash('sha256')
			.update('orchbee-desktop-development-endpoint-v1')
			.digest('hex')
			.slice(0, 32);
	const executable =
		input.platform === 'win32'
			? (input.environment.ComSpec ?? 'C:\\Windows\\System32\\cmd.exe')
			: (input.environment.SHELL ?? '/bin/sh');
	const recoveryDirectory = path.resolve(
		input.environment.ORCHBEE_DESKTOP_RECOVERY_DIRECTORY ??
			path.join(input.userDataPath, 'terminal-recovery'),
	);
	const gatewayFlag = input.environment.ORCHBEE_DESKTOP_GATEWAY;
	if (gatewayFlag !== undefined && gatewayFlag !== '0' && gatewayFlag !== '1') {
		throw new Error('ORCHBEE_DESKTOP_GATEWAY must be 0 or 1.');
	}
	const gateway: DesktopGatewayRuntimeConfiguration =
		gatewayFlag === '1'
			? {
					entryPath: path.resolve(
						input.environment.ORCHBEE_GATEWAY_ENTRY_PATH ??
							path.join(input.applicationPath, '..', 'gateway', 'dist', 'main', 'gateway.cjs'),
					),
					kind: 'enabled',
				}
			: { kind: 'disabled' };

	return {
		binaryPath,
		endpointId,
		gateway,
		runtimeDirectory,
		terminalBindingStorePath: path.join(recoveryDirectory, 'bindings-v3.json'),
		terminalLaunch: TerminalLaunchV1_1Schema.parse({
			arguments: [],
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: 80,
			currentDirectory: input.homeDirectory,
			environment: [
				{ name: 'COLORTERM', value: 'truecolor' },
				{ name: 'TERM', value: 'xterm-256color' },
			],
			executable,
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		}),
		workbenchDocumentStorePath: path.join(recoveryDirectory, 'workbench-shared-document-v1.json'),
	};
}
