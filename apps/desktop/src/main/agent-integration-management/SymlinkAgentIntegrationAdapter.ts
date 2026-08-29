import { lstat, mkdir, readlink, stat, symlink, unlink } from 'node:fs/promises';
import path from 'node:path';

import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';

export interface SymlinkAgentIntegrationAdapterOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly executableName: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly sourceKind: 'directory' | 'file';
	readonly sourcePath: string;
	readonly targetPath: string;
}

export class SymlinkAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #commandRunner: AgentIntegrationCommandRunner;
	readonly #executableName: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly #sourceKind: SymlinkAgentIntegrationAdapterOptions['sourceKind'];
	readonly #sourcePath: string;
	readonly #targetPath: string;

	constructor(options: SymlinkAgentIntegrationAdapterOptions) {
		this.#commandRunner = options.commandRunner;
		this.#executableName = options.executableName;
		this.integrationId = options.integrationId;
		this.managementMode = options.managementMode;
		this.#sourceKind = options.sourceKind;
		this.#sourcePath = path.resolve(options.sourcePath);
		this.#targetPath = path.resolve(options.targetPath);
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const [executablePath, sourceReady, targetState] = await Promise.all([
			this.#commandRunner.resolveExecutable(this.#executableName),
			sourceAvailable(this.#sourcePath, this.#sourceKind),
			inspectExactSymlink(this.#targetPath, this.#sourcePath),
		]);
		const platformIssue =
			this.managementMode === 'manual-only' ? ('unsupported-platform' as const) : null;
		let state: DesktopAgentIntegrationSnapshotItem['state'];
		if (targetState === 'missing') state = 'not-installed';
		else if (targetState === 'exact') state = 'enabled';
		else state = targetState;
		return {
			availability: executablePath === null ? 'not-found' : 'available',
			integrationId: this.integrationId,
			issue:
				platformIssue ?? (sourceReady && targetState !== 'unknown' ? null : 'inspection-failed'),
			managementMode: this.managementMode,
			state,
		};
	}

	async enable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		if (!(await sourceAvailable(this.#sourcePath, this.#sourceKind))) return { kind: 'failed' };
		const state = await inspectExactSymlink(this.#targetPath, this.#sourcePath);
		if (state === 'exact') return { kind: 'attempted' };
		if (state !== 'missing') return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		try {
			await mkdir(path.dirname(this.#targetPath), { mode: 0o700, recursive: true });
			await symlink(
				this.#sourcePath,
				this.#targetPath,
				this.#sourceKind === 'file' ? 'file' : 'dir',
			);
			return { kind: 'attempted' };
		} catch (error: unknown) {
			return { kind: isFileExistsError(error) ? 'conflict' : 'failed' };
		}
	}

	async disable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const state = await inspectExactSymlink(this.#targetPath, this.#sourcePath);
		if (state === 'missing') return { kind: 'attempted' };
		if (state !== 'exact') return { kind: state === 'conflict' ? 'conflict' : 'failed' };
		try {
			await unlink(this.#targetPath);
			return { kind: 'attempted' };
		} catch {
			return { kind: 'failed' };
		}
	}
}

type SymlinkState = 'conflict' | 'exact' | 'missing' | 'unknown';

async function inspectExactSymlink(
	targetPath: string,
	expectedSourcePath: string,
): Promise<SymlinkState> {
	try {
		const metadata = await lstat(targetPath);
		if (!metadata.isSymbolicLink()) return 'conflict';
		const linkedPath = await readlink(targetPath);
		const resolvedLinkedPath = path.resolve(path.dirname(targetPath), linkedPath);
		return resolvedLinkedPath === expectedSourcePath ? 'exact' : 'conflict';
	} catch (error: unknown) {
		return isFileNotFoundError(error) ? 'missing' : 'unknown';
	}
}

async function sourceAvailable(
	sourcePath: string,
	sourceKind: SymlinkAgentIntegrationAdapterOptions['sourceKind'],
): Promise<boolean> {
	try {
		const metadata = await stat(sourcePath);
		return sourceKind === 'file' ? metadata.isFile() : metadata.isDirectory();
	} catch {
		return false;
	}
}

function isFileExistsError(error: unknown): boolean {
	return isNodeError(error) && error.code === 'EEXIST';
}

function isFileNotFoundError(error: unknown): boolean {
	return isNodeError(error) && error.code === 'ENOENT';
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
	return error instanceof Error && 'code' in error;
}
