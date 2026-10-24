import { open } from 'node:fs/promises';
import path from 'node:path';

import type {
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import { runCommandSequence } from './agent-integration-command-operations';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';
import { classifyManagedResourceRoot } from './agent-integration-state-parsers';

const CODEX_MARKETPLACE_NAME = 'orchbee-local';
const CODEX_PLUGIN_ID = 'orchbee-agent-observation@orchbee-local';
const MAX_CODEX_CONFIGURATION_BYTES = 512 * 1024;

export interface CodexAgentIntegrationAdapterOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly homeDirectory: string;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly resourceRoot: string;
}

interface CodexConfigurationInspection {
	readonly marketplace: 'absent' | 'current' | 'update-required';
	readonly pluginPresent: boolean;
	readonly state:
		| 'conflict'
		| 'disabled'
		| 'enabled'
		| 'not-installed'
		| 'unknown'
		| 'update-required';
}

/** Uses Codex's owned TOML declarations so a broken local marketplace is still repairable. */
export class CodexAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #commandRunner: AgentIntegrationCommandRunner;
	readonly #configurationPath: string;
	readonly integrationId = 'codex' as const;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly #resourceRoot: string;

	constructor(options: CodexAgentIntegrationAdapterOptions) {
		this.#commandRunner = options.commandRunner;
		this.#configurationPath = path.resolve(options.homeDirectory, '.codex', 'config.toml');
		this.managementMode = options.managementMode;
		this.#resourceRoot = path.resolve(options.resourceRoot);
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const [executablePath, configuration] = await Promise.all([
			this.#commandRunner.resolveExecutable('codex'),
			this.#inspectConfiguration(),
		]);
		let issue: DesktopAgentIntegrationSnapshotItem['issue'] = null;
		if (this.managementMode === 'manual-only') issue = 'unsupported-platform';
		else if (configuration.state === 'unknown') issue = 'inspection-failed';
		return {
			availability: executablePath === null ? 'not-found' : 'available',
			integrationId: this.integrationId,
			issue,
			managementMode: this.managementMode,
			state: configuration.state,
		};
	}

	async enable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const executablePath = await this.#commandRunner.resolveExecutable('codex');
		if (executablePath === null) return { kind: 'failed' };
		const configuration = await this.#inspectConfiguration();
		if (configuration.state === 'enabled') return { kind: 'attempted' };
		if (configuration.state === 'conflict') return { kind: 'conflict' };
		if (configuration.state === 'unknown') return { kind: 'failed' };
		const commands: (readonly string[])[] = [];
		if (configuration.pluginPresent) {
			commands.push(['plugin', 'remove', CODEX_PLUGIN_ID, '--json']);
		}
		if (configuration.marketplace === 'update-required') {
			commands.push(['plugin', 'marketplace', 'remove', CODEX_MARKETPLACE_NAME, '--json']);
		}
		if (configuration.marketplace !== 'current') {
			commands.push(['plugin', 'marketplace', 'add', this.#resourceRoot, '--json']);
		}
		commands.push(['plugin', 'add', CODEX_PLUGIN_ID, '--json']);
		return runCommandSequence(
			{
				commandRunner: this.#commandRunner,
				executablePath,
				resourceRoot: this.#resourceRoot,
			},
			commands,
		);
	}

	async disable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const executablePath = await this.#commandRunner.resolveExecutable('codex');
		if (executablePath === null) return { kind: 'failed' };
		const configuration = await this.#inspectConfiguration();
		if (configuration.state === 'conflict') return { kind: 'conflict' };
		if (configuration.state === 'unknown') return { kind: 'failed' };
		const commands: (readonly string[])[] = [];
		if (configuration.pluginPresent) {
			commands.push(['plugin', 'remove', CODEX_PLUGIN_ID, '--json']);
		}
		if (configuration.marketplace === 'update-required') {
			commands.push(['plugin', 'marketplace', 'remove', CODEX_MARKETPLACE_NAME, '--json']);
		}
		return commands.length === 0
			? { kind: 'attempted' }
			: runCommandSequence(
					{
						commandRunner: this.#commandRunner,
						executablePath,
						resourceRoot: this.#resourceRoot,
					},
					commands,
				);
	}

	async #inspectConfiguration(): Promise<CodexConfigurationInspection> {
		const configuration = await readBoundedConfiguration(this.#configurationPath);
		if (configuration.kind === 'missing') {
			return { marketplace: 'absent', pluginPresent: false, state: 'not-installed' };
		}
		if (configuration.kind === 'unavailable') {
			return { marketplace: 'absent', pluginPresent: false, state: 'unknown' };
		}
		return parseCodexAgentIntegrationConfiguration(configuration.value, {
			resourceRoot: this.#resourceRoot,
		});
	}
}

function parseCodexAgentIntegrationConfiguration(
	value: string,
	options: { readonly resourceRoot: string },
): CodexConfigurationInspection {
	let section: 'marketplace' | 'other' | 'plugin' = 'other';
	let marketplaceCount = 0;
	let pluginCount = 0;
	const marketplaceValues = new Map<string, string>();
	const pluginValues = new Map<string, string>();
	for (const rawLine of value.split(/\r?\n/u)) {
		const line = rawLine.trim();
		if (line === '' || line.startsWith('#')) continue;
		if (line.startsWith('[')) {
			if (line === `[marketplaces.${CODEX_MARKETPLACE_NAME}]`) {
				marketplaceCount += 1;
				section = 'marketplace';
			} else if (line === `[plugins.${JSON.stringify(CODEX_PLUGIN_ID)}]`) {
				pluginCount += 1;
				section = 'plugin';
			} else {
				section = 'other';
			}
			continue;
		}
		if (section === 'other') continue;
		const assignment = /^([A-Za-z][A-Za-z0-9_]*)\s*=\s*(.+)$/u.exec(line);
		if (assignment === null) continue;
		const [, key, rawValue] = assignment;
		if (key === undefined || rawValue === undefined) continue;
		const target = section === 'marketplace' ? marketplaceValues : pluginValues;
		if (target.has(key)) {
			return { marketplace: 'absent', pluginPresent: false, state: 'conflict' };
		}
		target.set(key, rawValue.trim());
	}
	if (marketplaceCount > 1 || pluginCount > 1) {
		return { marketplace: 'absent', pluginPresent: false, state: 'conflict' };
	}
	const pluginPresent = pluginCount === 1;
	const enabled = pluginPresent ? parseBoolean(pluginValues.get('enabled')) : null;
	if (pluginPresent && enabled === null) {
		return { marketplace: 'absent', pluginPresent, state: 'conflict' };
	}
	if (marketplaceCount === 0) {
		return {
			marketplace: 'absent',
			pluginPresent,
			state: pluginPresent ? 'conflict' : 'not-installed',
		};
	}
	const sourceType = parseBasicString(marketplaceValues.get('source_type'));
	const source = parseBasicString(marketplaceValues.get('source'));
	if (sourceType !== 'local' || source === null || !path.isAbsolute(source)) {
		return { marketplace: 'absent', pluginPresent, state: 'conflict' };
	}
	const resolvedSource = path.resolve(source);
	if (resolvedSource === path.resolve(options.resourceRoot)) {
		let state: CodexConfigurationInspection['state'];
		if (!pluginPresent) state = 'not-installed';
		else state = enabled === true ? 'enabled' : 'disabled';
		return {
			marketplace: 'current',
			pluginPresent,
			state,
		};
	}
	if (
		classifyManagedResourceRoot(resolvedSource, {
			resourceRoot: options.resourceRoot,
		}) === 'update-required'
	) {
		return { marketplace: 'update-required', pluginPresent, state: 'update-required' };
	}
	return { marketplace: 'absent', pluginPresent, state: 'conflict' };
}

async function readBoundedConfiguration(
	filePath: string,
): Promise<
	| { readonly kind: 'available'; readonly value: string }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable' }
> {
	let handle: Awaited<ReturnType<typeof open>> | undefined;
	try {
		handle = await open(filePath, 'r');
		const metadata = await handle.stat();
		if (!metadata.isFile() || metadata.size > MAX_CODEX_CONFIGURATION_BYTES) {
			return { kind: 'unavailable' };
		}
		const bytes = Buffer.alloc(MAX_CODEX_CONFIGURATION_BYTES + 1);
		const { bytesRead } = await handle.read(bytes, 0, bytes.byteLength, 0);
		return bytesRead > MAX_CODEX_CONFIGURATION_BYTES
			? { kind: 'unavailable' }
			: { kind: 'available', value: bytes.subarray(0, bytesRead).toString('utf8') };
	} catch (error: unknown) {
		return isFileNotFoundError(error) ? { kind: 'missing' } : { kind: 'unavailable' };
	} finally {
		await handle?.close();
	}
}

function parseBasicString(value: string | undefined): string | null {
	if (value === undefined) return null;
	try {
		const parsed: unknown = JSON.parse(value);
		return typeof parsed === 'string' ? parsed : null;
	} catch {
		return null;
	}
}

function parseBoolean(value: string | undefined): boolean | null {
	if (value === 'true') return true;
	if (value === 'false') return false;
	return null;
}

function isFileNotFoundError(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}
