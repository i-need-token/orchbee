import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
	DesktopAgentIntegrationState,
} from '@orchbee/desktop-client-protocol';

import type { AgentIntegrationCommandRunner } from './NodeAgentIntegrationCommandRunner';
import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';

export interface CliAgentIntegrationOperationContext {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly executablePath: string;
	readonly resourceRoot: string;
}

export interface CliAgentIntegrationAdapterOptions {
	readonly commandRunner: AgentIntegrationCommandRunner;
	readonly disable: (
		context: CliAgentIntegrationOperationContext,
	) => Promise<AgentIntegrationMutationAttempt>;
	readonly enable: (
		context: CliAgentIntegrationOperationContext,
	) => Promise<AgentIntegrationMutationAttempt>;
	readonly executableName: string;
	readonly inspect: (
		context: CliAgentIntegrationOperationContext,
	) => Promise<DesktopAgentIntegrationState | null>;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly resourceRoot: string;
}

export class CliAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #commandRunner: AgentIntegrationCommandRunner;
	readonly #disableOperation: CliAgentIntegrationAdapterOptions['disable'];
	readonly #enableOperation: CliAgentIntegrationAdapterOptions['enable'];
	readonly #executableName: string;
	readonly #inspectOperation: CliAgentIntegrationAdapterOptions['inspect'];
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	readonly #resourceRoot: string;

	constructor(options: CliAgentIntegrationAdapterOptions) {
		this.#commandRunner = options.commandRunner;
		this.#disableOperation = options.disable;
		this.#enableOperation = options.enable;
		this.#executableName = options.executableName;
		this.#inspectOperation = options.inspect;
		this.integrationId = options.integrationId;
		this.managementMode = options.managementMode;
		this.#resourceRoot = options.resourceRoot;
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const executablePath = await this.#commandRunner.resolveExecutable(this.#executableName);
		if (executablePath === null) {
			return {
				availability: 'not-found',
				integrationId: this.integrationId,
				issue: this.managementMode === 'manual-only' ? 'unsupported-platform' : null,
				managementMode: this.managementMode,
				state: 'unknown',
			};
		}
		const state = await this.#inspectOperation(this.#context(executablePath));
		let issue: DesktopAgentIntegrationSnapshotItem['issue'] = null;
		if (this.managementMode === 'manual-only') issue = 'unsupported-platform';
		else if (state === null) issue = 'inspection-failed';
		return {
			availability: 'available',
			integrationId: this.integrationId,
			issue,
			managementMode: this.managementMode,
			state: state ?? 'unknown',
		};
	}

	enable(): Promise<AgentIntegrationMutationAttempt> {
		return this.#operate(this.#enableOperation);
	}

	disable(): Promise<AgentIntegrationMutationAttempt> {
		return this.#operate(this.#disableOperation);
	}

	async #operate(
		operation: CliAgentIntegrationAdapterOptions['enable'],
	): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return { kind: 'manual-action-required' };
		const executablePath = await this.#commandRunner.resolveExecutable(this.#executableName);
		if (executablePath === null) return { kind: 'failed' };
		return operation(this.#context(executablePath));
	}

	#context(executablePath: string): CliAgentIntegrationOperationContext {
		return {
			commandRunner: this.#commandRunner,
			executablePath,
			resourceRoot: this.#resourceRoot,
		};
	}
}
