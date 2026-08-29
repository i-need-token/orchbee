import path from 'node:path';

import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationManagementMode,
	type DesktopAgentIntegrationState,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import {
	CliAgentIntegrationAdapter,
	type CliAgentIntegrationOperationContext,
} from './CliAgentIntegrationAdapter';
import { SymlinkAgentIntegrationAdapter } from './SymlinkAgentIntegrationAdapter';
import { commandAttempt, runCommandSequence } from './agent-integration-command-operations';
import {
	CLAUDE_PLUGIN_ID,
	CODEX_PLUGIN_ID,
	GROK_PLUGIN_NAME,
	parseClaudeState,
	parseCodexState,
	parseGrokState,
	parseJsonRecord,
	parseRecordArray,
	readRecord,
	readRecordArray,
	readString,
} from './agent-integration-state-parsers';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';

const MARKETPLACE_NAME = 'orchbee-local';

export interface CreateDesktopAgentIntegrationAdaptersOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly homeDirectory: string;
	readonly platform: NodeJS.Platform;
	readonly resourceRoot: string;
}

export function createDesktopAgentIntegrationAdapters(
	options: CreateDesktopAgentIntegrationAdaptersOptions,
): readonly AgentIntegrationAdapter[] {
	const managementMode: DesktopAgentIntegrationManagementMode =
		options.platform === 'win32' ? 'manual-only' : 'managed-local';
	const bundleRoot = path.join(options.resourceRoot, 'bundle');
	const adapters: readonly AgentIntegrationAdapter[] = [
		new CliAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			disable: disableClaude,
			enable: enableClaude,
			executableName: 'claude',
			inspectArguments: ['plugin', 'list', '--json'],
			integrationId: 'claude-code',
			managementMode,
			parseState: parseClaudeState,
			resourceRoot: options.resourceRoot,
		}),
		new CliAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			disable: disableCodex,
			enable: enableCodex,
			executableName: 'codex',
			inspectArguments: ['plugin', 'list', '--json'],
			integrationId: 'codex',
			managementMode,
			parseState: parseCodexState,
			resourceRoot: options.resourceRoot,
		}),
		new CliAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			disable: disableGrok,
			enable: enableGrok,
			executableName: 'grok',
			inspectArguments: ['plugin', 'list', '--json'],
			integrationId: 'grok',
			managementMode,
			parseState: parseGrokState,
			resourceRoot: options.resourceRoot,
		}),
		new SymlinkAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			executableName: 'opencode',
			integrationId: 'opencode',
			managementMode,
			sourcePath: path.join(bundleRoot, 'opencode', 'orchbee-agent-observation.mjs'),
			targetPath: path.join(
				options.homeDirectory,
				'.config',
				'opencode',
				'plugins',
				'orchbee-agent-observation.mjs',
			),
		}),
		new SymlinkAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			executableName: 'pi',
			integrationId: 'pi',
			managementMode,
			sourcePath: path.join(bundleRoot, 'pi', 'orchbee-agent-observation.cjs'),
			targetPath: path.join(
				options.homeDirectory,
				'.pi',
				'agent',
				'extensions',
				'orchbee-agent-observation.cjs',
			),
		}),
		new SymlinkAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			executableName: 'omp',
			integrationId: 'omp',
			managementMode,
			sourcePath: path.join(bundleRoot, 'omp', 'orchbee-agent-observation.cjs'),
			targetPath: path.join(
				options.homeDirectory,
				'.omp',
				'agent',
				'extensions',
				'orchbee-agent-observation.cjs',
			),
		}),
	];
	if (
		adapters.length !== DESKTOP_AGENT_INTEGRATION_IDS.length ||
		!DESKTOP_AGENT_INTEGRATION_IDS.every(
			(integrationId, index) => adapters[index]?.integrationId === integrationId,
		)
	) {
		throw new Error('Desktop Agent integration registry is not canonical.');
	}
	return adapters;
}

async function enableClaude(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const state = await inspectState(context, ['plugin', 'list', '--json'], parseClaudeState);
	if (state === null) return { kind: 'failed' };
	if (state === 'enabled') return { kind: 'attempted' };
	if (state === 'disabled') {
		return commandAttempt(
			await context.commandRunner.run(context.executablePath, [
				'plugin',
				'enable',
				CLAUDE_PLUGIN_ID,
				'--scope',
				'user',
			]),
		);
	}
	if (state !== 'not-installed') return { kind: 'failed' };
	const marketplace = await ensureClaudeMarketplace(context);
	if (marketplace.kind !== 'attempted') return marketplace;
	return runCommandSequence(context, [
		['plugin', 'install', CLAUDE_PLUGIN_ID, '--scope', 'user'],
		['plugin', 'enable', CLAUDE_PLUGIN_ID, '--scope', 'user'],
	]);
}

async function disableClaude(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const state = await inspectState(context, ['plugin', 'list', '--json'], parseClaudeState);
	if (state === null) return { kind: 'failed' };
	if (state === 'not-installed' || state === 'disabled') return { kind: 'attempted' };
	if (state !== 'enabled') return { kind: 'failed' };
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'disable',
			CLAUDE_PLUGIN_ID,
			'--scope',
			'user',
		]),
	);
}

async function enableCodex(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const state = await inspectState(context, ['plugin', 'list', '--json'], parseCodexState);
	if (state === null) return { kind: 'failed' };
	if (state === 'enabled') return { kind: 'attempted' };
	const marketplace = await ensureCodexMarketplace(context);
	if (marketplace.kind !== 'attempted') return marketplace;
	const steps: (readonly string[])[] = [];
	if (state === 'disabled') {
		steps.push(['plugin', 'remove', CODEX_PLUGIN_ID, '--json']);
	}
	if (state !== 'disabled' && state !== 'not-installed') return { kind: 'failed' };
	steps.push(['plugin', 'add', CODEX_PLUGIN_ID, '--json']);
	return runCommandSequence(context, steps);
}

async function disableCodex(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const state = await inspectState(context, ['plugin', 'list', '--json'], parseCodexState);
	if (state === null) return { kind: 'failed' };
	if (state === 'not-installed') return { kind: 'attempted' };
	if (state !== 'enabled' && state !== 'disabled') return { kind: 'failed' };
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'remove',
			CODEX_PLUGIN_ID,
			'--json',
		]),
	);
}

async function enableGrok(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const state = await inspectState(context, ['plugin', 'list', '--json'], parseGrokState);
	if (state === null) return { kind: 'failed' };
	if (state === 'enabled') return { kind: 'attempted' };
	if (state === 'disabled') {
		return commandAttempt(
			await context.commandRunner.run(context.executablePath, [
				'plugin',
				'enable',
				GROK_PLUGIN_NAME,
			]),
		);
	}
	if (state !== 'not-installed') return { kind: 'failed' };
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'install',
			path.join(context.resourceRoot, 'bundle'),
			'--trust',
		]),
	);
}

async function disableGrok(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const state = await inspectState(context, ['plugin', 'list', '--json'], parseGrokState);
	if (state === null) return { kind: 'failed' };
	if (state === 'not-installed' || state === 'disabled') return { kind: 'attempted' };
	if (state !== 'enabled') return { kind: 'failed' };
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'disable',
			GROK_PLUGIN_NAME,
		]),
	);
}

async function ensureClaudeMarketplace(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const result = await context.commandRunner.run(context.executablePath, [
		'plugin',
		'marketplace',
		'list',
		'--json',
	]);
	if (result.kind !== 'completed') return commandAttempt(result);
	const entries = parseRecordArray(result.stdout);
	if (entries === null) return { kind: 'failed' };
	const existing = entries.find((entry) => readString(entry, 'name') === MARKETPLACE_NAME);
	if (existing !== undefined) {
		const sourceKind = readString(existing, 'source');
		const sourcePath = readString(existing, 'path');
		return (sourceKind === 'directory' || sourceKind === 'local') &&
			sourcePath !== null &&
			path.resolve(sourcePath) === path.resolve(context.resourceRoot)
			? { kind: 'attempted' }
			: { kind: 'conflict' };
	}
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'marketplace',
			'add',
			context.resourceRoot,
			'--scope',
			'user',
		]),
	);
}

async function ensureCodexMarketplace(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const result = await context.commandRunner.run(context.executablePath, [
		'plugin',
		'marketplace',
		'list',
		'--json',
	]);
	if (result.kind !== 'completed') return commandAttempt(result);
	const root = parseJsonRecord(result.stdout);
	const entries = root === null ? null : readRecordArray(root, 'marketplaces');
	if (entries === null) return { kind: 'failed' };
	const existing = entries.find((entry) => readString(entry, 'name') === MARKETPLACE_NAME);
	if (existing !== undefined) {
		const source = readRecord(existing, 'marketplaceSource');
		const sourceKind = source === null ? null : readString(source, 'sourceType');
		const sourcePath = source === null ? null : readString(source, 'source');
		return sourceKind === 'local' &&
			sourcePath !== null &&
			path.resolve(sourcePath) === path.resolve(context.resourceRoot)
			? { kind: 'attempted' }
			: { kind: 'conflict' };
	}
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'marketplace',
			'add',
			context.resourceRoot,
			'--json',
		]),
	);
}

async function inspectState(
	context: CliAgentIntegrationOperationContext,
	arguments_: readonly string[],
	parseState: (stdout: string) => DesktopAgentIntegrationState | null,
): Promise<DesktopAgentIntegrationState | null> {
	const result = await context.commandRunner.run(context.executablePath, arguments_);
	return result.kind === 'completed' ? parseState(result.stdout) : null;
}
