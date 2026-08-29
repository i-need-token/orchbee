import path from 'node:path';

import type { DesktopAgentIntegrationState } from '@orchbee/desktop-client-protocol';

export const CLAUDE_PLUGIN_ID = 'orchbee-agent-observation@orchbee-local';
export const CODEX_PLUGIN_ID = 'orchbee-agent-observation@orchbee-local';
export const GROK_PLUGIN_NAME = 'orchbee-agent-observation';
export const GITHUB_COPILOT_PLUGIN_ID = 'orchbee-agent-observation@orchbee-local';

export function parseClaudeState(stdout: string): DesktopAgentIntegrationState | null {
	const entries = parseRecordArray(stdout);
	if (entries === null) return null;
	const plugin = entries.find((entry) => readString(entry, 'id') === CLAUDE_PLUGIN_ID);
	if (plugin === undefined) return 'not-installed';
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled === null) return null;
	return enabled ? 'enabled' : 'disabled';
}

export function parseCodexState(stdout: string): DesktopAgentIntegrationState | null {
	const root = parseJsonRecord(stdout);
	const entries = root === null ? null : readRecordArray(root, 'installed');
	if (entries === null) return null;
	const plugin = entries.find((entry) => readString(entry, 'pluginId') === CODEX_PLUGIN_ID);
	if (plugin === undefined) return 'not-installed';
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled === null) return null;
	return enabled ? 'enabled' : 'disabled';
}

export function parseGrokState(stdout: string): DesktopAgentIntegrationState | null {
	const entries = parseRecordArray(stdout);
	if (entries === null) return null;
	const plugin = entries.find((entry) => readString(entry, 'name') === GROK_PLUGIN_NAME);
	if (plugin === undefined) return 'not-installed';
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled !== null) return enabled ? 'enabled' : 'disabled';
	return readString(plugin, 'status') === 'disabled' ? 'disabled' : 'enabled';
}

export function parseGitHubCopilotState(
	stdout: string,
	expectedResourceRoot: string,
): DesktopAgentIntegrationState | null {
	const root = parseJsonRecord(stdout);
	const entries = root === null ? null : readRecordArray(root, 'plugins');
	if (entries === null) return null;
	const matches = entries.filter(
		(entry) => readString(entry, 'name') === 'orchbee-agent-observation',
	);
	if (matches.length === 0) return 'not-installed';
	if (matches.length !== 1) return 'conflict';
	const [plugin] = matches;
	if (plugin === undefined) return null;
	const source = readString(plugin, 'source');
	const installedFrom = readString(plugin, 'installedFrom');
	if (
		source !== 'live-marketplace:orchbee-local' ||
		installedFrom === null ||
		path.resolve(installedFrom) !== path.resolve(expectedResourceRoot)
	) {
		return 'conflict';
	}
	const enabled = readBoolean(plugin, 'enabled');
	if (enabled === null) return null;
	return enabled ? 'enabled' : 'disabled';
}

type JsonRecord = Record<string, unknown>;

export function parseRecordArray(value: string): readonly JsonRecord[] | null {
	try {
		const parsed: unknown = JSON.parse(value);
		return Array.isArray(parsed) && parsed.every(isJsonRecord) ? parsed : null;
	} catch {
		return null;
	}
}

export function parseJsonRecord(value: string): JsonRecord | null {
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

export function readRecord(record: JsonRecord, key: string): JsonRecord | null {
	const value = record[key];
	return isJsonRecord(value) ? value : null;
}

export function readRecordArray(record: JsonRecord, key: string): readonly JsonRecord[] | null {
	const value = record[key];
	return Array.isArray(value) && value.every(isJsonRecord) ? value : null;
}

export function readString(record: JsonRecord, key: string): string | null {
	const value = record[key];
	return typeof value === 'string' ? value : null;
}

function isJsonRecord(value: unknown): value is JsonRecord {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
