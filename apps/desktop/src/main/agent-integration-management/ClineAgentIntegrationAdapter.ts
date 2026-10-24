import { lstat, open, readdir } from 'node:fs/promises';
import path from 'node:path';

import type {
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
	DesktopAgentIntegrationState,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import { commandAttempt } from './agent-integration-command-operations';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';

const CLINE_PLUGIN_NAME = 'orchbee-agent-observation';
const CLINE_CONTRACT_VERSION = 1;
const MAX_CLINE_INSTALLS = 128;
const MAX_MANIFEST_BYTES = 16 * 1024;

export interface ClineAgentIntegrationAdapterOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly homeDirectory: string;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly resourceRoot: string;
}

export class ClineAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #bundleRoot: string;
	readonly #commandRunner: AgentIntegrationCommandRunner;
	readonly #homeDirectory: string;
	readonly integrationId = 'cline' as const;
	readonly managementMode: DesktopAgentIntegrationManagementMode;

	constructor(options: ClineAgentIntegrationAdapterOptions) {
		this.#bundleRoot = path.resolve(options.resourceRoot, 'bundle');
		this.#commandRunner = options.commandRunner;
		this.#homeDirectory = path.resolve(options.homeDirectory);
		this.managementMode = options.managementMode;
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const [executablePath, sourceReady, state] = await Promise.all([
			this.#commandRunner.resolveExecutable('cline'),
			isManagedClinePackage(this.#bundleRoot),
			inspectManagedClineInstall(this.#homeDirectory),
		]);
		let issue: DesktopAgentIntegrationSnapshotItem['issue'] = null;
		if (this.managementMode === 'manual-only') issue = 'unsupported-platform';
		else if (state === 'unknown') issue = 'inspection-failed';
		const observedState =
			(state === 'enabled' && !sourceReady) || state === 'update-required'
				? 'update-required'
				: state;
		return {
			availability: executablePath === null ? 'not-found' : 'available',
			integrationId: this.integrationId,
			issue,
			managementMode: this.managementMode,
			state: observedState,
		};
	}

	async enable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		if (!(await isManagedClinePackage(this.#bundleRoot))) return { kind: 'failed' };
		const state = await inspectManagedClineInstall(this.#homeDirectory);
		if (state === 'enabled') return { kind: 'attempted' };
		if (state !== 'not-installed' && state !== 'update-required') {
			return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		}
		const executablePath = await this.#commandRunner.resolveExecutable('cline');
		if (executablePath === null) return { kind: 'failed' };
		if (state === 'update-required') {
			const removal = await this.#commandRunner.run(executablePath, [
				'plugin',
				'uninstall',
				CLINE_PLUGIN_NAME,
				'--json',
			]);
			if (removal.kind !== 'completed') return commandAttempt(removal);
		}
		return commandAttempt(
			await this.#commandRunner.run(executablePath, [
				'plugin',
				'install',
				this.#bundleRoot,
				'--json',
			]),
		);
	}

	async disable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const state = await inspectManagedClineInstall(this.#homeDirectory);
		if (state === 'not-installed') return { kind: 'attempted' };
		if (state !== 'enabled' && state !== 'update-required') {
			return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		}
		const executablePath = await this.#commandRunner.resolveExecutable('cline');
		if (executablePath === null) return { kind: 'failed' };
		return commandAttempt(
			await this.#commandRunner.run(executablePath, [
				'plugin',
				'uninstall',
				CLINE_PLUGIN_NAME,
				'--json',
			]),
		);
	}
}

async function inspectManagedClineInstall(
	homeDirectory: string,
): Promise<DesktopAgentIntegrationState> {
	const pluginRoot = path.join(homeDirectory, '.cline', 'plugins');
	for (const directName of [
		`${CLINE_PLUGIN_NAME}.js`,
		`${CLINE_PLUGIN_NAME}.ts`,
		CLINE_PLUGIN_NAME,
	]) {
		const directState = await pathState(path.join(pluginRoot, directName));
		if (directState === 'present' || directState === 'symlink') return 'conflict';
		if (directState === 'unknown') return 'unknown';
	}

	const installedRoot = path.join(pluginRoot, '_installed', 'local');
	let entries;
	try {
		entries = await readdir(installedRoot, { withFileTypes: true });
	} catch (error: unknown) {
		return isFileNotFoundError(error) ? 'not-installed' : 'unknown';
	}
	if (entries.length > MAX_CLINE_INSTALLS) return 'unknown';
	let exactCount = 0;
	let updateRequiredCount = 0;
	for (const entry of entries) {
		if (!entry.isDirectory()) continue;
		const candidateRoot = path.join(installedRoot, entry.name, 'package');
		const entryState = await pathState(
			path.join(candidateRoot, 'cline', 'orchbee-agent-observation.js'),
		);
		if (entryState === 'missing') continue;
		if (entryState === 'unknown') return 'unknown';
		if (entryState === 'symlink') return 'conflict';
		const packageState = await inspectManagedClinePackage(candidateRoot);
		if (packageState === 'conflict') return 'conflict';
		if (packageState === 'unknown') return 'unknown';
		if (packageState === 'update-required') updateRequiredCount += 1;
		else exactCount += 1;
	}
	if (exactCount + updateRequiredCount > 1) return 'conflict';
	if (updateRequiredCount === 1) return 'update-required';
	return exactCount === 1 ? 'enabled' : 'not-installed';
}

async function isManagedClinePackage(packageRoot: string): Promise<boolean> {
	return (await inspectManagedClinePackage(packageRoot)) === 'current';
}

type ManagedClinePackageState = 'conflict' | 'current' | 'unknown' | 'update-required';

async function inspectManagedClinePackage(packageRoot: string): Promise<ManagedClinePackageState> {
	try {
		if (!(await lstat(packageRoot)).isDirectory()) return 'conflict';
	} catch {
		return 'unknown';
	}
	const value = await readJsonRecord(path.join(packageRoot, 'package.json'));
	if (value === null) return 'conflict';
	if (value['name'] !== CLINE_PLUGIN_NAME) return 'conflict';
	const marker = isRecord(value['orchbeeAgentObservation'])
		? value['orchbeeAgentObservation']
		: null;
	const contractVersion = marker?.['contractVersion'];
	if (contractVersion === CLINE_CONTRACT_VERSION) return 'current';
	return Number.isSafeInteger(contractVersion) && Number(contractVersion) > 0
		? 'update-required'
		: 'conflict';
}

async function readJsonRecord(filePath: string): Promise<Record<string, unknown> | null> {
	let handle: Awaited<ReturnType<typeof open>> | undefined;
	try {
		if (!(await lstat(filePath)).isFile()) return null;
		handle = await open(filePath, 'r');
		const metadata = await handle.stat();
		if (!metadata.isFile() || metadata.size > MAX_MANIFEST_BYTES) return null;
		const bytes = Buffer.alloc(MAX_MANIFEST_BYTES + 1);
		const { bytesRead } = await handle.read(bytes, 0, bytes.byteLength, 0);
		if (bytesRead > MAX_MANIFEST_BYTES) return null;
		const parsed: unknown = JSON.parse(bytes.subarray(0, bytesRead).toString('utf8'));
		return isRecord(parsed) ? parsed : null;
	} catch {
		return null;
	} finally {
		await handle?.close();
	}
}

type PathState = 'missing' | 'present' | 'symlink' | 'unknown';

async function pathState(filePath: string): Promise<PathState> {
	try {
		return (await lstat(filePath)).isSymbolicLink() ? 'symlink' : 'present';
	} catch (error: unknown) {
		return isFileNotFoundError(error) ? 'missing' : 'unknown';
	}
}

function isFileNotFoundError(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'ENOENT';
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
