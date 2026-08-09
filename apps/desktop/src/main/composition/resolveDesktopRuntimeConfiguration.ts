import { createHash } from 'node:crypto';
import path from 'node:path';

import {
	DEFAULT_TERMINAL_CONFIGURATION,
	TerminalConfigurationSchema,
	TerminalLaunchSchema,
	type TerminalConfiguration,
	type TerminalLaunch,
} from '@orchbee/terminal-protocol';

import { createDefaultTerminalProfile } from '../host';

export interface DesktopRuntimeEnvironment {
	readonly LANG?: string;
	readonly LC_ALL?: string;
	readonly LC_CTYPE?: string;
	readonly ORCHBEE_DESKTOP_ENDPOINT_ID?: string;
	readonly ORCHBEE_DESKTOP_GATEWAY?: string;
	readonly ORCHBEE_DESKTOP_RECOVERY_DIRECTORY?: string;
	readonly ORCHBEE_DESKTOP_RUNTIME_DIRECTORY?: string;
	readonly ORCHBEE_GATEWAY_ENTRY_PATH?: string;
	readonly ORCHBEE_TERMINAL_HOST_BINARY?: string;
	readonly ORCHBEE_TERMINAL_SCROLLBACK_BYTES?: string;
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
	readonly packagedResourcesPath?: string | undefined;
	readonly platform: NodeJS.Platform;
	readonly userDataPath: string;
}

export interface DesktopRuntimeConfiguration {
	readonly binaryPath: string;
	readonly endpointId: string;
	readonly gateway: DesktopGatewayRuntimeConfiguration;
	readonly runtimeDirectory: string;
	readonly terminalBindingStorePath: string;
	readonly terminalExitPublicationLedgerPath: string;
	readonly terminalExitSpillPath: string;
	readonly terminalConfiguration: TerminalConfiguration;
	readonly terminalLaunch: TerminalLaunch;
	readonly workbenchDocumentStorePath: string;
}

export function resolveDesktopRuntimeConfiguration(
	input: DesktopRuntimeConfigurationInput,
): DesktopRuntimeConfiguration {
	const binaryName =
		input.platform === 'win32' ? 'orchbee-terminal-host.exe' : 'orchbee-terminal-host';
	const configuredBinary = input.environment.ORCHBEE_TERMINAL_HOST_BINARY;
	const configuredRuntimeDirectory = input.environment.ORCHBEE_DESKTOP_RUNTIME_DIRECTORY;
	const productScopeDigest = createHash('sha256')
		.update('orchbee-terminal-host-product-scope-v1\0')
		.update(input.userDataPath)
		.digest('hex');
	let binaryPath: string;
	if (configuredBinary !== undefined) {
		binaryPath = path.resolve(configuredBinary);
	} else if (input.packagedResourcesPath !== undefined) {
		binaryPath = path.resolve(input.packagedResourcesPath, binaryName);
	} else {
		binaryPath = path.resolve(
			input.applicationPath,
			'..',
			'terminal-host',
			'zig-out',
			'bin',
			binaryName,
		);
	}
	let runtimeDirectory: string;
	if (configuredRuntimeDirectory !== undefined) {
		runtimeDirectory = path.resolve(configuredRuntimeDirectory);
	} else if (input.platform === 'win32') {
		runtimeDirectory = path.join(input.userDataPath, 'terminal-host');
	} else {
		runtimeDirectory = path.join('/tmp', `orchbee-desktop-${productScopeDigest.slice(0, 12)}`);
	}
	const endpointId =
		input.environment.ORCHBEE_DESKTOP_ENDPOINT_ID ?? productScopeDigest.slice(0, 32);
	const configuredScrollbackBytes = input.environment.ORCHBEE_TERMINAL_SCROLLBACK_BYTES;
	let terminalConfiguration = DEFAULT_TERMINAL_CONFIGURATION;
	if (configuredScrollbackBytes !== undefined) {
		if (!/^(?:0|[1-9][0-9]*)$/u.test(configuredScrollbackBytes)) {
			throw new Error('ORCHBEE_TERMINAL_SCROLLBACK_BYTES must be a decimal byte count.');
		}
		const scrollbackBytes = Number(configuredScrollbackBytes);
		const parsed = TerminalConfigurationSchema.safeParse({ scrollbackBytes });
		if (!parsed.success) {
			throw new Error('ORCHBEE_TERMINAL_SCROLLBACK_BYTES exceeds the supported range.');
		}
		terminalConfiguration = parsed.data;
	}
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
	const terminalLocaleEnvironment = resolveTerminalLocaleEnvironment(
		input.environment,
		input.platform,
	);

	return {
		binaryPath,
		endpointId,
		gateway,
		runtimeDirectory,
		terminalBindingStorePath: path.join(recoveryDirectory, 'bindings-v3.json'),
		terminalExitPublicationLedgerPath: path.join(
			recoveryDirectory,
			'terminal-exit-publications-v1.json',
		),
		terminalExitSpillPath: path.join(recoveryDirectory, 'terminal-exits-v1.json'),
		terminalConfiguration,
		terminalLaunch: TerminalLaunchSchema.parse({
			arguments: [],
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: 80,
			configuration: null,
			currentDirectory: input.homeDirectory,
			environment: [
				{ name: 'COLORTERM', value: 'truecolor' },
				{ name: 'TERM', value: 'xterm-256color' },
				...terminalLocaleEnvironment,
			],
			executable,
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		}),
		workbenchDocumentStorePath: path.join(recoveryDirectory, 'workbench-shared-document-v1.json'),
	};
}

function resolveTerminalLocaleEnvironment(
	environment: DesktopRuntimeEnvironment,
	platform: NodeJS.Platform,
): TerminalLaunch['environment'] {
	if (platform === 'win32') return [];

	const effectiveLocale =
		nonEmpty(environment.LC_ALL) ??
		nonEmpty(environment.LC_CTYPE) ??
		nonEmpty(environment.LANG);
	if (effectiveLocale !== undefined && isSupportedUtf8Locale(effectiveLocale, platform)) {
		return [];
	}

	const fallbackLocale = platform === 'darwin' ? 'en_US.UTF-8' : 'C.UTF-8';
	return [
		...(nonEmpty(environment.LC_ALL) === undefined
			? []
			: [{ name: 'LC_ALL', value: null } as const]),
		{ name: 'LC_CTYPE', value: fallbackLocale },
	];
}

function nonEmpty(value: string | undefined): string | undefined {
	return value === undefined || value.length === 0 ? undefined : value;
}

function isSupportedUtf8Locale(value: string, platform: NodeJS.Platform): boolean {
	const localeWithoutModifier = value.split('@', 1)[0];
	if (localeWithoutModifier === undefined || !/(?:^|\.)utf-?8$/iu.test(localeWithoutModifier)) {
		return false;
	}

	return !(
		platform === 'darwin' && localeWithoutModifier.replaceAll('-', '').toLowerCase() === 'c.utf8'
	);
}
