import { createHash } from 'node:crypto';
import { accessSync, constants as fileSystemConstants } from 'node:fs';
import path from 'node:path';

import {
	DEFAULT_TERMINAL_CONFIGURATION,
	TerminalConfigurationSchema,
	TerminalLaunchSchema,
	type TerminalConfiguration,
	type TerminalLaunch,
} from '@orchbee/terminal-protocol';

import { createDefaultTerminalProfile, POSIX_HOST_ENDPOINT_ADDON_NAME } from '../host';
import { AGENT_OBSERVATION_CAPSULE_VERSION } from '../agent-integration-management';
import type {
	AutomaticShellIntegrationConfiguration,
	AutomaticShellIntegrationShell,
} from '../terminal-launch-profiles';
import { SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME } from '../semantic-command';

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
	readonly XDG_DATA_DIRS?: string;
	readonly ZDOTDIR?: string;
	readonly ComSpec?: string;
}

export type DesktopBrowserAccessRuntimeConfiguration =
	| { readonly kind: 'disabled' }
	| {
			readonly gatewayEntryPath: string;
			readonly kind: 'enabled';
			readonly personalAccessTokenStorePath: string;
			readonly rendererRoot: string;
			readonly serverEntryPath: string;
	  };

export interface DesktopRuntimeConfigurationInput {
	readonly applicationPath: string;
	readonly darwinSupportedLocales?: readonly string[] | undefined;
	readonly darwinSystemLocale?: string | undefined;
	readonly developmentAgentObservationAdapterDirectory?: string | undefined;
	readonly developmentPosixHostEndpointAddonPath?: string | undefined;
	readonly developmentSemanticCommandEndpointAddonPath?: string | undefined;
	readonly developmentShellIntegrationDirectory?: string | undefined;
	readonly environment: DesktopRuntimeEnvironment;
	readonly homeDirectory: string;
	readonly packagedResourcesPath?: string | undefined;
	readonly platform: NodeJS.Platform;
	readonly posixRuntimeBaseDirectory?: string | undefined;
	readonly terminalHostName: string;
	readonly userDataPath: string;
}

export interface DesktopRuntimeConfiguration {
	readonly agentObservationAdapterDirectory: string;
	readonly agentObservationCapsuleDirectory: string;
	readonly automaticShellIntegration: AutomaticShellIntegrationConfiguration | null;
	readonly binaryPath: string;
	readonly browserAccess: DesktopBrowserAccessRuntimeConfiguration;
	readonly endpointId: string;
	readonly environmentRegistryLegacyStorePath: string;
	readonly environmentRegistryStorePath: string;
	readonly homeDirectory: string;
	readonly managedWorktreeRoot: string;
	readonly posixHostEndpointAddonPath: string | null;
	readonly projectWorktreeOperationStorePath: string;
	readonly runtimeDirectory: string;
	readonly semanticCommandEndpointAddonPath: string;
	readonly terminalArchiveDirectory: string;
	readonly terminalBindingStorePath: string;
	readonly terminalExitPublicationLedgerPath: string;
	readonly terminalExitSpillPath: string;
	readonly terminalConfiguration: TerminalConfiguration;
	readonly terminalDefaultSettingsStorePath: string;
	readonly terminalHostName: string;
	readonly terminalHostLaunchMode: 'connect-only' | 'connect-or-launch';
	readonly terminalLaunch: TerminalLaunch;
	readonly terminalLaunchProfileStorePath: string;
	readonly windowStateStorePath: string;
	readonly workbenchDocumentLegacyStorePath: string;
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
	let shellIntegrationDirectory: string | null;
	if (input.platform === 'win32') {
		shellIntegrationDirectory = null;
	} else if (input.packagedResourcesPath !== undefined) {
		shellIntegrationDirectory = path.resolve(input.packagedResourcesPath, 'shell-integration');
	} else {
		shellIntegrationDirectory = path.resolve(
			input.developmentShellIntegrationDirectory ??
				path.join(input.applicationPath, 'resources', 'shell-integration'),
		);
	}
	const agentObservationAdapterDirectory =
		input.packagedResourcesPath === undefined
			? path.resolve(
					input.developmentAgentObservationAdapterDirectory ??
						path.join(input.applicationPath, 'resources', 'agent-observation-adapters'),
				)
			: path.resolve(input.packagedResourcesPath, 'agent-observation-adapters');
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
				personalAccessTokenStorePath: path.join(
					input.userDataPath,
					'personal-access-token-v1.json',
				),
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
		input.darwinSupportedLocales,
		input.darwinSystemLocale,
	);
	const supervisedHostFlag = input.environment.ORCHBEE_TERMINAL_HOST_SUPERVISED;
	if (
		supervisedHostFlag !== undefined &&
		supervisedHostFlag !== '0' &&
		supervisedHostFlag !== '1'
	) {
		throw new Error('ORCHBEE_TERMINAL_HOST_SUPERVISED must be 0 or 1.');
	}
	let posixHostEndpointAddonPath: string | null;
	if (input.platform === 'win32') {
		posixHostEndpointAddonPath = null;
	} else if (input.packagedResourcesPath === undefined) {
		posixHostEndpointAddonPath = path.resolve(
			input.developmentPosixHostEndpointAddonPath ??
				path.join(input.applicationPath, 'zig-out', 'lib', POSIX_HOST_ENDPOINT_ADDON_NAME),
		);
	} else {
		posixHostEndpointAddonPath = path.resolve(
			input.packagedResourcesPath,
			POSIX_HOST_ENDPOINT_ADDON_NAME,
		);
	}
	const semanticCommandEndpointAddonPath =
		input.packagedResourcesPath === undefined
			? path.resolve(
					input.developmentSemanticCommandEndpointAddonPath ??
						path.join(
							input.applicationPath,
							'zig-out',
							'lib',
							SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME,
						),
				)
			: path.resolve(input.packagedResourcesPath, SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME);
	// Cross-application artifact contract: search for `semantic-command-cli-launcher-name` before changing these values.
	const semanticCommandCliLauncherName =
		input.platform === 'win32' ? 'orchbee-cli.cmd' : 'orchbee-cli';
	const semanticCommandCliPlatformPath = input.platform === 'win32' ? path.win32 : path.posix;
	const semanticCommandCliLauncherPath =
		input.packagedResourcesPath === undefined
			? semanticCommandCliPlatformPath.resolve(
					input.applicationPath,
					'..',
					'cli',
					'resources',
					semanticCommandCliLauncherName,
				)
			: semanticCommandCliPlatformPath.resolve(
					input.packagedResourcesPath,
					semanticCommandCliLauncherName,
				);

	return {
		agentObservationAdapterDirectory,
		agentObservationCapsuleDirectory: path.join(
			input.userDataPath,
			'agent-observation-adapters',
			AGENT_OBSERVATION_CAPSULE_VERSION,
		),
		automaticShellIntegration: resolveAutomaticShellIntegrationConfiguration(
			shellIntegrationDirectory,
			input.environment,
		),
		binaryPath,
		browserAccess,
		endpointId,
		environmentRegistryLegacyStorePath: path.join(
			input.userDataPath,
			'environment-registry-v1.json',
		),
		environmentRegistryStorePath: path.join(input.userDataPath, 'environment-registry-v2.json'),
		homeDirectory: input.homeDirectory,
		managedWorktreeRoot: path.join(input.userDataPath, 'worktrees'),
		posixHostEndpointAddonPath,
		projectWorktreeOperationStorePath: path.join(
			input.userDataPath,
			'project-worktree-operations-v1.json',
		),
		runtimeDirectory,
		semanticCommandEndpointAddonPath,
		terminalArchiveDirectory: path.join(recoveryDirectory, 'terminal-archives-v1'),
		terminalBindingStorePath: path.join(recoveryDirectory, 'bindings-v3.json'),
		terminalExitPublicationLedgerPath: path.join(
			recoveryDirectory,
			'terminal-exit-publications-v1.json',
		),
		terminalExitSpillPath: path.join(recoveryDirectory, 'terminal-exits-v1.json'),
		terminalConfiguration,
		terminalDefaultSettingsStorePath: path.join(
			input.userDataPath,
			'terminal-default-settings-v1.json',
		),
		terminalHostName: input.terminalHostName,
		terminalHostLaunchMode: supervisedHostFlag === '1' ? 'connect-only' : 'connect-or-launch',
		terminalLaunch: TerminalLaunchSchema.parse({
			arguments: [],
			cellPixelHeight: null,
			cellPixelWidth: null,
			columns: null,
			configuration: null,
			currentDirectory: input.homeDirectory,
			environment: [
				{ name: 'COLORTERM', value: 'truecolor' },
				{ name: 'TERM', value: 'xterm-256color' },
				{ name: 'TERM_PROGRAM', value: 'ghostty' },
				{ name: 'ORCHBEE_CLI', value: semanticCommandCliLauncherPath },
				{ name: 'ORCHBEE_CLI_RUNTIME_DIRECTORY', value: runtimeDirectory },
				...(shellIntegrationDirectory === null
					? []
					: [{ name: 'ORCHBEE_SHELL_INTEGRATION_DIR', value: shellIntegrationDirectory }]),
				{ name: 'NO_COLOR', value: null },
				{ name: 'FORCE_COLOR', value: null },
				{ name: 'NODE_DISABLE_COLORS', value: null },
				...terminalLocaleEnvironment,
			],
			executable,
			rows: null,
			terminalProfile: createDefaultTerminalProfile(),
		}),
		terminalLaunchProfileStorePath: path.join(
			input.userDataPath,
			'terminal-launch-profiles-v1.json',
		),
		windowStateStorePath: path.join(input.userDataPath, 'window-state-v1.json'),
		workbenchDocumentLegacyStorePath: path.join(
			recoveryDirectory,
			'workbench-shared-document-v1.json',
		),
		workbenchDocumentStorePath: path.join(recoveryDirectory, 'workbench-shared-document-v2.json'),
	};
}

function resolveAutomaticShellIntegrationConfiguration(
	directory: string | null,
	environment: DesktopRuntimeEnvironment,
): AutomaticShellIntegrationConfiguration | null {
	if (directory === null) return null;
	const resourcePaths = {
		bash: ['orchbee.bash', path.join('automatic', 'bash', 'orchbee.bash')],
		fish: [
			'orchbee.fish',
			path.join('automatic', 'fish', 'fish', 'vendor_conf.d', 'orchbee-shell-integration.fish'),
		],
		zsh: ['orchbee.zsh', path.join('automatic', 'zsh', '.zshenv')],
	} satisfies Record<AutomaticShellIntegrationShell, readonly string[]>;
	const supportedShells: readonly AutomaticShellIntegrationShell[] = ['bash', 'fish', 'zsh'];
	const availableShells = supportedShells.filter((shell) =>
		resourcePaths[shell].every((relativePath) =>
			isReadableResource(path.join(directory, relativePath)),
		),
	);
	if (availableShells.length === 0) return null;
	return {
		availableShells,
		directory,
		inheritedEnvironment: {
			xdgDataDirectories: environment.XDG_DATA_DIRS ?? null,
			zdotdir: environment.ZDOTDIR ?? null,
		},
	};
}

function isReadableResource(filePath: string): boolean {
	try {
		accessSync(filePath, fileSystemConstants.R_OK);
		return true;
	} catch {
		return false;
	}
}

function resolveTerminalLocaleEnvironment(
	environment: DesktopRuntimeEnvironment,
	platform: NodeJS.Platform,
	darwinSupportedLocales: readonly string[] | undefined,
	darwinSystemLocale: string | undefined,
): TerminalLaunch['environment'] {
	if (platform === 'win32') return [];

	const lcAll = nonEmpty(environment.LC_ALL);
	const lang = nonEmpty(environment.LANG);
	const lcCtype = nonEmpty(environment.LC_CTYPE);
	if (platform === 'darwin') {
		const fallbackLocale = resolveDarwinUtf8Locale(darwinSystemLocale, darwinSupportedLocales);
		const resolvedLcAll =
			lcAll !== undefined && isSupportedUtf8Locale(lcAll, platform) ? lcAll : null;
		const resolvedLocale =
			[lcCtype, lang].find(
				(value): value is string => value !== undefined && isSupportedUtf8Locale(value, platform),
			) ?? fallbackLocale;
		let resolvedLang = fallbackLocale;
		if (lang !== undefined && isSupportedUtf8Locale(lang, platform)) {
			resolvedLang = lang;
		} else if (resolvedLcAll === null) {
			resolvedLang = resolvedLocale;
		}
		let resolvedLcCtype: string | null = null;
		if (lcCtype !== undefined) {
			resolvedLcCtype = isSupportedUtf8Locale(lcCtype, platform) ? lcCtype : resolvedLocale;
		}

		return [
			{ name: 'LC_ALL', value: resolvedLcAll },
			{ name: 'LANG', value: resolvedLang },
			{ name: 'LC_CTYPE', value: resolvedLcCtype },
		];
	}

	const fallbackLocale = 'C.UTF-8';
	if (lcAll !== undefined && isSupportedUtf8Locale(lcAll, platform)) {
		return [];
	}
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

function resolveDarwinUtf8Locale(
	systemLocale: string | undefined,
	supportedLocales: readonly string[] | undefined,
): string {
	if (systemLocale !== undefined && systemLocale.length > 0) {
		try {
			const locale = new Intl.Locale(systemLocale);
			const region = locale.region;
			if (
				/^[a-z]{2,3}$/u.test(locale.language) &&
				region !== undefined &&
				/^[A-Z]{2}$/u.test(region)
			) {
				const candidate = `${locale.language}_${region}.UTF-8`;
				const normalizedCandidate = candidate.toLowerCase();
				if (
					supportedLocales?.some(
						(supportedLocale) => supportedLocale.toLowerCase() === normalizedCandidate,
					) === true
				) {
					return candidate;
				}
			}
		} catch {
			// Electron can return an empty or non-BCP-47 value when the native locale is unavailable.
		}
	}
	return 'en_US.UTF-8';
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
