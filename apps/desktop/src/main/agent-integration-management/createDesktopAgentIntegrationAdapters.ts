import path from 'node:path';

import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationManagementMode,
	type DesktopAgentIntegrationState,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import { CapsuleAgentIntegrationAdapter } from './CapsuleAgentIntegrationAdapter';
import {
	CliAgentIntegrationAdapter,
	type CliAgentIntegrationOperationContext,
} from './CliAgentIntegrationAdapter';
import { ClineAgentIntegrationAdapter } from './ClineAgentIntegrationAdapter';
import { CodexAgentIntegrationAdapter } from './CodexAgentIntegrationAdapter';
import {
	FileAgentObservationAdapterCapsule,
	type AgentObservationAdapterCapsule,
} from './file-agent-observation-adapter-capsule';
import { GuardedLocalAgentIntegrationAdapter } from './GuardedLocalAgentIntegrationAdapter';
import { commandAttempt, runCommandSequence } from './agent-integration-command-operations';
import {
	CLAUDE_PLUGIN_ID,
	GITHUB_COPILOT_PLUGIN_ID,
	GROK_PLUGIN_NAME,
	parseClaudeConfiguration,
	parseGrokConfiguration,
	parseGitHubCopilotConfiguration,
	type ClaudeAgentIntegrationConfiguration,
	type GitHubCopilotAgentIntegrationConfiguration,
	type GrokAgentIntegrationConfiguration,
} from './agent-integration-state-parsers';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';

const MARKETPLACE_NAME = 'orchbee-local';

export interface CreateDesktopAgentIntegrationAdaptersOptions {
	readonly capsule?: AgentObservationAdapterCapsule;
	readonly capsuleRoot: string;
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
	const capsule =
		options.capsule ??
		new FileAgentObservationAdapterCapsule({
			rootPath: options.capsuleRoot,
			sourcePath: options.resourceRoot,
		});
	const stableResourceRoot = capsule.rootPath;
	const bundleRoot = path.join(stableResourceRoot, 'bundle');
	const sharedRuntimeRoot = path.join(bundleRoot, 'scripts');
	const adapters = [
		new CliAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			disable: disableClaude,
			enable: enableClaude,
			executableName: 'claude',
			inspect: inspectClaudeState,
			integrationId: 'claude-code',
			managementMode,
			resourceRoot: stableResourceRoot,
		}),
		new CodexAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			homeDirectory: options.homeDirectory,
			managementMode,
			resourceRoot: stableResourceRoot,
		}),
		new CliAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			disable: disableGrok,
			enable: enableGrok,
			executableName: 'grok',
			inspect: inspectGrokState,
			integrationId: 'grok',
			managementMode,
			resourceRoot: stableResourceRoot,
		}),
		new GuardedLocalAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			entryKind: 'opencode-module',
			executableName: 'opencode',
			integrationId: 'opencode',
			managementMode,
			resourceRoot: stableResourceRoot,
			sourcePath: path.join(bundleRoot, 'opencode', 'orchbee-agent-observation.mjs'),
			targetPath: path.join(
				options.homeDirectory,
				'.config',
				'opencode',
				'plugins',
				'orchbee-agent-observation.mjs',
			),
		}),
		new GuardedLocalAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			entryKind: 'pi-module',
			executableName: 'pi',
			integrationId: 'pi',
			managementMode,
			resourceRoot: stableResourceRoot,
			sourcePath: path.join(bundleRoot, 'pi', 'orchbee-agent-observation.cjs'),
			targetPath: path.join(
				options.homeDirectory,
				'.pi',
				'agent',
				'extensions',
				'orchbee-agent-observation.cjs',
			),
		}),
		new GuardedLocalAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			entryKind: 'omp-module',
			executableName: 'omp',
			integrationId: 'omp',
			managementMode,
			resourceRoot: stableResourceRoot,
			sourcePath: path.join(bundleRoot, 'omp', 'orchbee-agent-observation.cjs'),
			targetPath: path.join(
				options.homeDirectory,
				'.omp',
				'agent',
				'extensions',
				'orchbee-agent-observation.cjs',
			),
		}),
		new GuardedLocalAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			entryKind: 'extension-directory',
			executableName: 'gemini',
			integrationId: 'gemini',
			managementMode,
			resourceRoot: stableResourceRoot,
			runtimeSourcePath: sharedRuntimeRoot,
			sourcePath: path.join(bundleRoot, 'gemini'),
			targetPath: path.join(
				options.homeDirectory,
				'.gemini',
				'extensions',
				'orchbee-agent-observation',
			),
		}),
		new CliAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			disable: disableGitHubCopilot,
			enable: enableGitHubCopilot,
			executableName: 'copilot',
			inspect: inspectGitHubCopilotState,
			integrationId: 'github-copilot',
			managementMode,
			resourceRoot: stableResourceRoot,
		}),
		new ClineAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			homeDirectory: options.homeDirectory,
			managementMode,
			resourceRoot: stableResourceRoot,
		}),
		new GuardedLocalAgentIntegrationAdapter({
			commandRunner: options.commandRunner,
			entryKind: 'extension-directory',
			executableName: 'qwen',
			integrationId: 'qwen-code',
			managementMode,
			resourceRoot: stableResourceRoot,
			runtimeSourcePath: sharedRuntimeRoot,
			sourcePath: path.join(bundleRoot, 'qwen-code'),
			targetPath: path.join(
				options.homeDirectory,
				'.qwen',
				'extensions',
				'orchbee-agent-observation',
			),
		}),
	].map((adapter) => new CapsuleAgentIntegrationAdapter(adapter, capsule));
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
	const configuration = await inspectClaudeConfiguration(context);
	if (configuration === null) return { kind: 'failed' };
	if (configuration.state === 'enabled') return { kind: 'attempted' };
	if (configuration.state === 'conflict') return { kind: 'conflict' };
	if (configuration.state === 'disabled') {
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
	if (configuration.state !== 'not-installed' && configuration.state !== 'update-required') {
		return { kind: 'failed' };
	}
	const commands: (readonly string[])[] = [];
	if (configuration.state === 'update-required' && configuration.pluginPresent) {
		commands.push(['plugin', 'uninstall', CLAUDE_PLUGIN_ID, '--scope', 'user']);
	}
	if (configuration.marketplace === 'update-required') {
		commands.push(['plugin', 'marketplace', 'remove', MARKETPLACE_NAME, '--scope', 'user']);
	}
	if (configuration.marketplace !== 'current') {
		commands.push(['plugin', 'marketplace', 'add', context.resourceRoot, '--scope', 'user']);
	}
	commands.push(
		['plugin', 'install', CLAUDE_PLUGIN_ID, '--scope', 'user'],
		['plugin', 'enable', CLAUDE_PLUGIN_ID, '--scope', 'user'],
	);
	return runCommandSequence(context, commands);
}

async function disableClaude(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const configuration = await inspectClaudeConfiguration(context);
	if (configuration === null) return { kind: 'failed' };
	if (configuration.state === 'not-installed' || configuration.state === 'disabled') {
		return { kind: 'attempted' };
	}
	if (configuration.state === 'conflict') return { kind: 'conflict' };
	if (configuration.state === 'enabled') {
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
	if (configuration.state !== 'update-required') return { kind: 'failed' };
	const commands: (readonly string[])[] = [];
	if (configuration.pluginPresent) {
		commands.push(['plugin', 'uninstall', CLAUDE_PLUGIN_ID, '--scope', 'user']);
	}
	if (configuration.marketplace === 'update-required') {
		commands.push(['plugin', 'marketplace', 'remove', MARKETPLACE_NAME, '--scope', 'user']);
	}
	return commands.length === 0 ? { kind: 'failed' } : runCommandSequence(context, commands);
}

async function enableGrok(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const configuration = await inspectGrokConfiguration(context);
	if (configuration === null) return { kind: 'failed' };
	if (configuration.state === 'enabled') return { kind: 'attempted' };
	if (configuration.state === 'conflict') return { kind: 'conflict' };
	if (configuration.state === 'disabled') {
		return commandAttempt(
			await context.commandRunner.run(context.executablePath, [
				'plugin',
				'enable',
				GROK_PLUGIN_NAME,
			]),
		);
	}
	if (configuration.state !== 'not-installed' && configuration.state !== 'update-required') {
		return { kind: 'failed' };
	}
	const commands: (readonly string[])[] = [];
	if (configuration.pluginPresent) {
		commands.push(['plugin', 'uninstall', GROK_PLUGIN_NAME, '--confirm']);
	}
	commands.push(['plugin', 'install', path.join(context.resourceRoot, 'bundle'), '--trust']);
	return runCommandSequence(context, commands);
}

async function disableGrok(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const configuration = await inspectGrokConfiguration(context);
	if (configuration === null) return { kind: 'failed' };
	if (configuration.state === 'not-installed' || configuration.state === 'disabled') {
		return { kind: 'attempted' };
	}
	if (configuration.state === 'conflict') return { kind: 'conflict' };
	if (!configuration.pluginPresent) return { kind: 'failed' };
	return commandAttempt(
		await context.commandRunner.run(context.executablePath, [
			'plugin',
			'uninstall',
			GROK_PLUGIN_NAME,
			'--confirm',
		]),
	);
}

async function enableGitHubCopilot(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const configuration = await inspectGitHubCopilotConfiguration(context);
	if (configuration === null) return { kind: 'failed' };
	if (configuration.state === 'enabled') return { kind: 'attempted' };
	if (configuration.state === 'conflict') return { kind: 'conflict' };
	if (configuration.state === 'disabled') {
		return commandAttempt(
			await context.commandRunner.run(context.executablePath, [
				'plugins',
				'enable',
				GITHUB_COPILOT_PLUGIN_ID,
				'--plugin',
			]),
		);
	}
	if (configuration.state !== 'not-installed' && configuration.state !== 'update-required') {
		return { kind: 'failed' };
	}
	const commands: (readonly string[])[] = [];
	if (configuration.pluginPresent) {
		commands.push(['plugins', 'remove', GITHUB_COPILOT_PLUGIN_ID, '--plugin']);
	}
	if (configuration.marketplace === 'update-required') {
		commands.push(['plugins', 'marketplace', 'remove', MARKETPLACE_NAME]);
	}
	if (configuration.marketplace !== 'current') {
		commands.push(['plugins', 'marketplace', 'add', context.resourceRoot]);
	}
	commands.push(['plugins', 'install', GITHUB_COPILOT_PLUGIN_ID]);
	return runCommandSequence(context, commands);
}

async function disableGitHubCopilot(
	context: CliAgentIntegrationOperationContext,
): Promise<AgentIntegrationMutationAttempt> {
	const configuration = await inspectGitHubCopilotConfiguration(context);
	if (configuration === null) return { kind: 'failed' };
	if (configuration.state === 'not-installed' || configuration.state === 'disabled') {
		return { kind: 'attempted' };
	}
	if (configuration.state === 'conflict') return { kind: 'conflict' };
	if (configuration.state === 'enabled') {
		return commandAttempt(
			await context.commandRunner.run(context.executablePath, [
				'plugins',
				'disable',
				GITHUB_COPILOT_PLUGIN_ID,
				'--plugin',
			]),
		);
	}
	if (configuration.state !== 'update-required') return { kind: 'failed' };
	const commands: (readonly string[])[] = [];
	if (configuration.pluginPresent) {
		commands.push(['plugins', 'remove', GITHUB_COPILOT_PLUGIN_ID, '--plugin']);
	}
	if (configuration.marketplace === 'update-required') {
		commands.push(['plugins', 'marketplace', 'remove', MARKETPLACE_NAME]);
	}
	return commands.length === 0 ? { kind: 'failed' } : runCommandSequence(context, commands);
}

async function inspectClaudeState(
	context: CliAgentIntegrationOperationContext,
): Promise<DesktopAgentIntegrationState | null> {
	return (await inspectClaudeConfiguration(context))?.state ?? null;
}

async function inspectClaudeConfiguration(
	context: CliAgentIntegrationOperationContext,
): Promise<ClaudeAgentIntegrationConfiguration | null> {
	const pluginResult = await context.commandRunner.run(context.executablePath, [
		'plugin',
		'list',
		'--json',
	]);
	if (pluginResult.kind !== 'completed') return null;
	const marketplaceResult = await context.commandRunner.run(context.executablePath, [
		'plugin',
		'marketplace',
		'list',
		'--json',
	]);
	return marketplaceResult.kind === 'completed'
		? parseClaudeConfiguration(
				pluginResult.stdout,
				marketplaceResult.stdout,
				resourceOptions(context),
			)
		: null;
}

async function inspectGrokState(
	context: CliAgentIntegrationOperationContext,
): Promise<DesktopAgentIntegrationState | null> {
	return (await inspectGrokConfiguration(context))?.state ?? null;
}

async function inspectGrokConfiguration(
	context: CliAgentIntegrationOperationContext,
): Promise<GrokAgentIntegrationConfiguration | null> {
	const result = await context.commandRunner.run(context.executablePath, [
		'plugin',
		'list',
		'--json',
	]);
	return result.kind === 'completed'
		? parseGrokConfiguration(result.stdout, resourceOptions(context))
		: null;
}

async function inspectGitHubCopilotState(
	context: CliAgentIntegrationOperationContext,
): Promise<DesktopAgentIntegrationState | null> {
	return (await inspectGitHubCopilotConfiguration(context))?.state ?? null;
}

async function inspectGitHubCopilotConfiguration(
	context: CliAgentIntegrationOperationContext,
): Promise<GitHubCopilotAgentIntegrationConfiguration | null> {
	const pluginResult = await context.commandRunner.run(context.executablePath, [
		'plugins',
		'list',
		'--kind',
		'plugin',
		'--json',
	]);
	if (pluginResult.kind !== 'completed') return null;
	const marketplaceResult = await context.commandRunner.run(context.executablePath, [
		'plugins',
		'marketplace',
		'list',
		'--json',
	]);
	return marketplaceResult.kind === 'completed'
		? parseGitHubCopilotConfiguration(
				pluginResult.stdout,
				marketplaceResult.stdout,
				resourceOptions(context),
			)
		: null;
}

function resourceOptions(context: CliAgentIntegrationOperationContext) {
	return {
		resourceRoot: context.resourceRoot,
	};
}
