import { createHash } from 'node:crypto';
import path from 'node:path';

import {
	CreationIdSchema,
	CreateTerminalV1_1Schema,
	type CreateTerminalV1_1,
} from '@orchbee/terminal-protocol';

import { createDefaultTerminalProfile } from '../host';

export interface DesktopRuntimeEnvironment {
	readonly ORCHBEE_DESKTOP_ENDPOINT_ID?: string;
	readonly ORCHBEE_DESKTOP_RUNTIME_DIRECTORY?: string;
	readonly ORCHBEE_TERMINAL_HOST_BINARY?: string;
	readonly SHELL?: string;
	readonly ComSpec?: string;
}

export interface DesktopRuntimeConfigurationInput {
	readonly applicationPath: string;
	readonly environment: DesktopRuntimeEnvironment;
	readonly homeDirectory: string;
	readonly platform: NodeJS.Platform;
	readonly userDataPath: string;
}

export interface DesktopRuntimeConfiguration {
	readonly binaryPath: string;
	readonly createTerminal: CreateTerminalV1_1;
	readonly endpointId: string;
	readonly runtimeDirectory: string;
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
		runtimeDirectory = path.join(input.userDataPath, 'terminal-host-slice-5');
	} else {
		runtimeDirectory = path.join(
			'/tmp',
			`orchbee-desktop-${createHash('sha256').update(input.homeDirectory).digest('hex').slice(0, 12)}`,
		);
	}
	const endpointId =
		input.environment.ORCHBEE_DESKTOP_ENDPOINT_ID ??
		createHash('sha256')
			.update('orchbee-desktop-slice-5-development-endpoint-v1')
			.digest('hex')
			.slice(0, 32);
	const creationId = CreationIdSchema.parse(
		createHash('sha256')
			.update(`orchbee-desktop-slice-5-terminal-v1:${endpointId}`)
			.digest('hex')
			.slice(0, 32),
	);
	const executable =
		input.platform === 'win32'
			? (input.environment.ComSpec ?? 'C:\\Windows\\System32\\cmd.exe')
			: (input.environment.SHELL ?? '/bin/sh');

	return {
		binaryPath,
		createTerminal: CreateTerminalV1_1Schema.parse({
			creationId,
			launch: {
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
			},
		}),
		endpointId,
		runtimeDirectory,
	};
}
