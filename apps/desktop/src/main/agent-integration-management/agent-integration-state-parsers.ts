import path from 'node:path';

import type { DesktopAgentIntegrationState } from '@orchbee/desktop-client-protocol';

export const CLAUDE_PLUGIN_ID = 'orchbee-agent-observation@orchbee-local';
export const GROK_PLUGIN_NAME = 'orchbee-agent-observation';
export const GITHUB_COPILOT_PLUGIN_ID = 'orchbee-agent-observation@orchbee-local';

const MARKETPLACE_NAME = 'orchbee-local';

export interface ManagedAgentIntegrationResourceOptions {
	readonly expectedVersion: string;
	readonly legacyResourceRoots: readonly string[];
	readonly resourceRoot: string;
}

export interface ClaudeAgentIntegrationConfiguration {
	readonly marketplace: ManagedMarketplaceState;
	readonly pluginPresent: boolean;
	readonly state: DesktopAgentIntegrationState;
}

export interface GrokAgentIntegrationConfiguration {
	readonly pluginPresent: boolean;
	readonly state: DesktopAgentIntegrationState;
}

export interface GitHubCopilotAgentIntegrationConfiguration {
	readonly marketplace: ManagedMarketplaceState;
	readonly pluginPresent: boolean;
	readonly state: DesktopAgentIntegrationState;
}

type ManagedMarketplaceState = 'absent' | 'current' | 'legacy';

export function parseClaudeConfiguration(
	pluginOutput: string,
	marketplaceOutput: string,
	options: ManagedAgentIntegrationResourceOptions,
): ClaudeAgentIntegrationConfiguration | null {
	const plugins = parseRecordArray(pluginOutput);
	const marketplaces = parseRecordArray(marketplaceOutput);
	if (plugins === null || marketplaces === null) return null;
	const pluginMatches = plugins.filter((entry) => readString(entry, 'id') === CLAUDE_PLUGIN_ID);
	const marketplaceMatches = marketplaces.filter(
		(entry) => readString(entry, 'name') === MARKETPLACE_NAME,
	);
	if (pluginMatches.length > 1 || marketplaceMatches.length > 1) {
		return { marketplace: 'absent', pluginPresent: false, state: 'conflict' };
	}
	const plugin = pluginMatches[0];
	const marketplace = classifyClaudeMarketplace(marketplaceMatches[0], options);
	if (marketplace === 'foreign') {
		return { marketplace: 'absent', pluginPresent: plugin !== undefined, state: 'conflict' };
	}
	if (marketplace === 'legacy') {
		return { marketplace, pluginPresent: plugin !== undefined, state: 'update-required' };
	}
	if (plugin === undefined) {
		return { marketplace, pluginPresent: false, state: 'not-installed' };
	}
	if (marketplace === 'absent') {
		return { marketplace, pluginPresent: true, state: 'conflict' };
	}
	const version = readString(plugin, 'version');
	if (version === null) return null;
	if (version !== options.expectedVersion) {
		return { marketplace, pluginPresent: true, state: 'update-required' };
	}
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled === null) return null;
	return { marketplace, pluginPresent: true, state: enabled ? 'enabled' : 'disabled' };
}

export function parseGrokConfiguration(
	stdout: string,
	options: ManagedAgentIntegrationResourceOptions,
): GrokAgentIntegrationConfiguration | null {
	const entries = parseRecordArray(stdout);
	if (entries === null) return null;
	const matches = entries.filter((entry) => readString(entry, 'name') === GROK_PLUGIN_NAME);
	if (matches.length === 0) return { pluginPresent: false, state: 'not-installed' };
	if (matches.length !== 1) return { pluginPresent: false, state: 'conflict' };
	const plugin = matches[0];
	if (plugin === undefined) return null;
	const source = readString(plugin, 'source');
	const version = readString(plugin, 'version');
	if (source === null || version === null || path.basename(source) !== 'bundle') {
		return { pluginPresent: true, state: 'conflict' };
	}
	const ownership = classifyManagedResourceRoot(path.dirname(source), options);
	if (ownership === 'foreign') return { pluginPresent: true, state: 'conflict' };
	if (ownership === 'legacy' || version !== options.expectedVersion) {
		return { pluginPresent: true, state: 'update-required' };
	}
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled !== null) {
		return { pluginPresent: true, state: enabled ? 'enabled' : 'disabled' };
	}
	const status = readString(plugin, 'status');
	if (status === 'disabled') return { pluginPresent: true, state: 'disabled' };
	if (status === 'enabled' || status === 'installed') {
		return { pluginPresent: true, state: 'enabled' };
	}
	return null;
}

export function parseGitHubCopilotConfiguration(
	pluginOutput: string,
	marketplaceOutput: string,
	options: ManagedAgentIntegrationResourceOptions,
): GitHubCopilotAgentIntegrationConfiguration | null {
	const root = parseJsonRecord(pluginOutput);
	const plugins = root === null ? null : readRecordArray(root, 'plugins');
	const marketplaces = parseRecordArray(marketplaceOutput);
	if (plugins === null || marketplaces === null) return null;
	const pluginMatches = plugins.filter((entry) => readString(entry, 'name') === GROK_PLUGIN_NAME);
	const marketplaceMatches = marketplaces.filter(
		(entry) => readString(entry, 'name') === MARKETPLACE_NAME,
	);
	if (pluginMatches.length > 1 || marketplaceMatches.length > 1) {
		return { marketplace: 'absent', pluginPresent: false, state: 'conflict' };
	}
	const plugin = pluginMatches[0];
	const marketplace = classifyGitHubCopilotMarketplace(marketplaceMatches[0], options);
	if (marketplace === 'foreign') {
		return { marketplace: 'absent', pluginPresent: plugin !== undefined, state: 'conflict' };
	}
	if (plugin === undefined) {
		return {
			marketplace,
			pluginPresent: false,
			state: marketplace === 'legacy' ? 'update-required' : 'not-installed',
		};
	}
	const source = readString(plugin, 'source');
	const installedFrom = readString(plugin, 'installedFrom');
	if (source !== 'live-marketplace:orchbee-local' || installedFrom === null) {
		return { marketplace, pluginPresent: true, state: 'conflict' };
	}
	const ownership = classifyManagedResourceRoot(installedFrom, options);
	if (ownership === 'foreign' || marketplace === 'absent') {
		return { marketplace, pluginPresent: true, state: 'conflict' };
	}
	if (ownership === 'legacy' || marketplace === 'legacy') {
		return { marketplace, pluginPresent: true, state: 'update-required' };
	}
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled === null) return null;
	return { marketplace, pluginPresent: true, state: enabled ? 'enabled' : 'disabled' };
}

export function classifyManagedResourceRoot(
	value: string,
	options: Pick<ManagedAgentIntegrationResourceOptions, 'legacyResourceRoots' | 'resourceRoot'>,
): 'current' | 'foreign' | 'legacy' {
	if (!path.isAbsolute(value)) return 'foreign';
	const resolved = path.resolve(value);
	const current = path.resolve(options.resourceRoot);
	if (resolved === current) return 'current';
	if (options.legacyResourceRoots.some((candidate) => path.resolve(candidate) === resolved)) {
		return 'legacy';
	}
	const normalized = resolved.split(path.sep).join('/');
	if (
		/(?:^|\/)Orchbee(?: Dev| Local)?\.app\/Contents\/Resources\/agent-observation-adapters$/u.test(
			normalized,
		) ||
		normalized.endsWith('/apps/desktop/resources/agent-observation-adapters')
	) {
		return 'legacy';
	}
	const versionPattern = /^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.-]+)?$/u;
	if (
		path.dirname(resolved) === path.dirname(current) &&
		versionPattern.test(path.basename(resolved)) &&
		versionPattern.test(path.basename(current))
	) {
		return 'legacy';
	}
	return 'foreign';
}

type JsonRecord = Record<string, unknown>;
type MarketplaceOwnership = ManagedMarketplaceState | 'foreign';

function parseRecordArray(value: string): readonly JsonRecord[] | null {
	try {
		const parsed: unknown = JSON.parse(value);
		return Array.isArray(parsed) && parsed.every(isJsonRecord) ? parsed : null;
	} catch {
		return null;
	}
}

function parseJsonRecord(value: string): JsonRecord | null {
	try {
		const parsed: unknown = JSON.parse(value);
		return isJsonRecord(parsed) ? parsed : null;
	} catch {
		return null;
	}
}

function readBoolean(record: JsonRecord, key: string): boolean | null {
	const value = record[key];
	return typeof value === 'boolean' ? value : null;
}

function readRecordArray(record: JsonRecord, key: string): readonly JsonRecord[] | null {
	const value = record[key];
	return Array.isArray(value) && value.every(isJsonRecord) ? value : null;
}

function readString(record: JsonRecord, key: string): string | null {
	const value = record[key];
	return typeof value === 'string' ? value : null;
}

function classifyClaudeMarketplace(
	entry: JsonRecord | undefined,
	options: ManagedAgentIntegrationResourceOptions,
): MarketplaceOwnership {
	if (entry === undefined) return 'absent';
	const sourceKind = readString(entry, 'source');
	const sourcePath = readString(entry, 'path');
	if ((sourceKind !== 'directory' && sourceKind !== 'local') || sourcePath === null) {
		return 'foreign';
	}
	return classifyManagedResourceRoot(sourcePath, options);
}

function classifyGitHubCopilotMarketplace(
	entry: JsonRecord | undefined,
	options: ManagedAgentIntegrationResourceOptions,
): MarketplaceOwnership {
	if (entry === undefined) return 'absent';
	const source = readString(entry, 'source');
	const prefix = 'Local: ';
	if (source === null || !source.startsWith(prefix)) return 'foreign';
	return classifyManagedResourceRoot(source.slice(prefix.length), options);
}

function isJsonRecord(value: unknown): value is JsonRecord {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
