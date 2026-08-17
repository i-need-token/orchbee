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
	readonly FORCE_COLOR?: string;
	readonly LANG?: string;
	readonly LC_ALL?: string;
	readonly LC_CTYPE?: string;
	readonly NODE_DISABLE_COLORS?: string;
	readonly NO_COLOR?: string;
	readonly ORCHBEE_DESKTOP_ENDPOINT_ID?: string;
	readonly ORCHBEE_DESKTOP_GATEWAY?: string;
	readonly ORCHBEE_DESKTOP_RECOVERY_DIRECTORY?: string;
	readonly ORCHBEE_DESKTOP_RUNTIME_DIRECTORY?: string;
	readonly ORCHBEE_GATEWAY_ENTRY_PATH?: string;
	readonly ORCHBEE_SERVER_ENTRY_PATH?: string;
	readonly ORCHBEE_TERMINAL_HOST_BINARY?: string;
	readonly ORCHBEE_TERMINAL_HOST_SUPERVISED?: string;
	readonly ORCHBEE_TERMINAL_SCROLLBACK_BYTES?: string;
	readonly ORCHBEE_WEB_RENDERER_ROOT?: string;
	readonly SHELL?: string;
	readonly ComSpec?: string;
}

export type DesktopBrowserAccessRuntimeConfiguration =
	| { readonly kind: 'disabled' }
	| {
			readonly gatewayEntryPath: string;
			readonly kind: 'enabled';
			readonly rendererRoot: string;
			readonly serverEntryPath: string;
	  };

export interface DesktopRuntimeConfigurationInput {
	readonly applicationPath: string;
	readonly environment: DesktopRuntimeEnvironment;
	readonly homeDirectory: string;
	readonly packagedResourcesPath?: string | undefined;
	readonly platform: NodeJS.Platform;
	readonly posixRuntimeBaseDirectory?: string | undefined;
	readonly terminalHostName: string;
	readonly userDataPath: string;
}

export interface DesktopRuntimeConfiguration {
	readonly binaryPath: string;
	readonly browserAccess: DesktopBrowserAccessRuntimeConfiguration;
	readonly endpointId: string;
	readonly runtimeDirectory: string;
	readonly terminalBindingStorePath: string;
	readonly terminalExitPublicationLedgerPath: string;
	readonly terminalExitSpillPath: string;
	readonly terminalConfiguration: TerminalConfiguration;
	readonly terminalHostName: string;
	readonly terminalHostLaunchMode: 'connect-only' | 'connect-or-launch';
	readonly terminalLaunch: TerminalLaunch;
	readonly windowStateStorePath: string;
	readonly workbenchDocumentStorePath: string;
}

export function resolveDesktopRuntimeConfiguration(
	input: DesktopRuntimeConfigurationInput,
): DesktopRuntimeConfiguration {
	if (input.terminalHostName.length === 0 || input.terminalHostName.length > 253) {
		throw new Error('A trusted terminal hostname is required.');
	}
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
		const runtimeBaseDirectory = input.posixRuntimeBaseDirectory;
		if (runtimeBaseDirectory === undefined) {
			throw new Error('A trusted POSIX runtime base directory is required.');
		}
		const resolvedRuntimeBase = path.resolve(runtimeBaseDirectory);
		if (
			resolvedRuntimeBase === '/tmp' ||
			resolvedRuntimeBase.startsWith('/tmp/') ||
			resolvedRuntimeBase === '/private/tmp' ||
			resolvedRuntimeBase.startsWith('/private/tmp/')
		) {
			throw new Error('The default POSIX runtime base must not be under shared temporary storage.');
		}
		runtimeDirectory = path.join(resolvedRuntimeBase, `ob-${productScopeDigest.slice(0, 12)}`);
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
	const browserAccessEnabled =
		gatewayFlag === '1' || (gatewayFlag === undefined && input.packagedResourcesPath !== undefined);
	const browserAccess: DesktopBrowserAccessRuntimeConfiguration = browserAccessEnabled
		? {
				gatewayEntryPath: path.resolve(
					input.environment.ORCHBEE_GATEWAY_ENTRY_PATH ??
						(input.packagedResourcesPath === undefined
							? path.join(input.applicationPath, '..', 'gateway', 'dist', 'main', 'gateway.cjs')
							: path.join(input.packagedResourcesPath, 'orchbee-gateway.cjs')),
				),
				kind: 'enabled',
				rendererRoot: path.resolve(
					input.environment.ORCHBEE_WEB_RENDERER_ROOT ??
						(input.packagedResourcesPath === undefined
							? path.join(input.applicationPath, '..', 'web', 'dist')
							: path.join(input.packagedResourcesPath, 'dist')),
				),
				serverEntryPath: path.resolve(
					input.environment.ORCHBEE_SERVER_ENTRY_PATH ??
						(input.packagedResourcesPath === undefined
							? path.join(input.applicationPath, '..', 'server', 'dist', 'main', 'server.cjs')
							: path.join(input.packagedResourcesPath, 'orchbee-server.cjs')),
				),
			}
		: { kind: 'disabled' };
	const terminalLocaleEnvironment = resolveTerminalLocaleEnvironment(
		input.environment,
		input.platform,
	);
	const supervisedHostFlag = input.environment.ORCHBEE_TERMINAL_HOST_SUPERVISED;
	if (
		supervisedHostFlag !== undefined &&
		supervisedHostFlag !== '0' &&
		supervisedHostFlag !== '1'
	) {
		throw new Error('ORCHBEE_TERMINAL_HOST_SUPERVISED must be 0 or 1.');
	}

	return {
		binaryPath,
		browserAccess,
		endpointId,
		runtimeDirectory,
		terminalBindingStorePath: path.join(recoveryDirectory, 'bindings-v3.json'),
		terminalExitPublicationLedgerPath: path.join(
			recoveryDirectory,
			'terminal-exit-publications-v1.json',
		),
		terminalExitSpillPath: path.join(recoveryDirectory, 'terminal-exits-v1.json'),
		terminalConfiguration,
		terminalHostName: input.terminalHostName,
		terminalHostLaunchMode: supervisedHostFlag === '1' ? 'connect-only' : 'connect-or-launch',
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
				{ name: 'TERM_PROGRAM', value: 'Orchbee' },
				{ name: 'NO_COLOR', value: null },
				{ name: 'FORCE_COLOR', value: null },
				{ name: 'NODE_DISABLE_COLORS', value: null },
				...terminalLocaleEnvironment,
			],
			executable,
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		}),
		windowStateStorePath: path.join(input.userDataPath, 'window-state-v1.json'),
		workbenchDocumentStorePath: path.join(recoveryDirectory, 'workbench-shared-document-v1.json'),
	};
}

function resolveTerminalLocaleEnvironment(
	environment: DesktopRuntimeEnvironment,
	platform: NodeJS.Platform,
): TerminalLaunch['environment'] {
	if (platform === 'win32') return [];

	const lcAll = nonEmpty(environment.LC_ALL);
	if (lcAll !== undefined && isSupportedUtf8Locale(lcAll, platform)) return [];
	const lcCtype = nonEmpty(environment.LC_CTYPE);
	const lang = nonEmpty(environment.LANG);
	const fallbackLocale = platform === 'darwin' ? 'en_US.UTF-8' : 'C.UTF-8';
	const resolvedLocale =
		[lcCtype, lang].find(
			(value): value is string => value !== undefined && isSupportedUtf8Locale(value, platform),
		) ?? fallbackLocale;
	const langNeedsRepair = lang !== undefined && !isSupportedUtf8Locale(lang, platform);
	const lcCtypeNeedsRepair =
		lcCtype === undefined
			? lang === undefined || langNeedsRepair
			: !isSupportedUtf8Locale(lcCtype, platform);

	return [
		...(lcAll === undefined ? [] : [{ name: 'LC_ALL', value: null } as const]),
		...(langNeedsRepair ? [{ name: 'LANG', value: resolvedLocale } as const] : []),
		...(lcCtypeNeedsRepair ? [{ name: 'LC_CTYPE', value: resolvedLocale } as const] : []),
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
