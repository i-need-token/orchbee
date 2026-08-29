import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationAvailability,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationManagementMode,
	type DesktopAgentIntegrationState,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from '../agent-integration-adapter';
import { DesktopAgentIntegrationManagementController } from '../DesktopAgentIntegrationManagementController';

describe('DesktopAgentIntegrationManagementController', () => {
	it('initializes through read-only inspection in canonical order', async () => {
		const adapters = createAdapters();
		const controller = new DesktopAgentIntegrationManagementController(adapters);

		const snapshot = await controller.initialize();

		expect(snapshot.integrations.map(({ integrationId }) => integrationId)).toEqual(
			DESKTOP_AGENT_INTEGRATION_IDS,
		);
		expect(adapters.every((candidate) => candidate.enableCount === 0)).toBe(true);
		expect(adapters.every((candidate) => candidate.disableCount === 0)).toBe(true);
	});

	it('mutates only after an explicit request and reports success after verification', async () => {
		const adapters = createAdapters();
		const codex = adapter(adapters, 'codex');
		codex.enableEffect = () => {
			codex.state = 'enabled';
			return Promise.resolve({ kind: 'attempted' });
		};
		const controller = new DesktopAgentIntegrationManagementController(adapters);
		await controller.initialize();

		const result = await controller.enable('codex');

		expect(result.kind).toBe('completed');
		expect(codex.enableCount).toBe(1);
		expect(
			result.snapshot.integrations.find(({ integrationId }) => integrationId === 'codex')?.state,
		).toBe('enabled');
	});

	it('never invokes a mutation for a conflicting owned path', async () => {
		const adapters = createAdapters();
		const opencode = adapter(adapters, 'opencode');
		opencode.state = 'conflict';
		const controller = new DesktopAgentIntegrationManagementController(adapters);
		await controller.initialize();

		const result = await controller.enable('opencode');

		expect(result).toMatchObject({ code: 'conflict', kind: 'failed' });
		expect(opencode.enableCount).toBe(0);
	});

	it('does not claim success when post-operation inspection misses the desired state', async () => {
		const adapters = createAdapters();
		const grok = adapter(adapters, 'grok');
		grok.enableEffect = () => Promise.resolve({ kind: 'attempted' });
		const controller = new DesktopAgentIntegrationManagementController(adapters);
		await controller.initialize();

		const result = await controller.enable('grok');

		expect(result).toMatchObject({ code: 'verification-failed', kind: 'failed' });
		expect(grok.state).toBe('not-installed');
	});

	it('allows disabling an exact integration after the Agent executable disappears', async () => {
		const adapters = createAdapters();
		const pi = adapter(adapters, 'pi');
		pi.availability = 'not-found';
		pi.state = 'enabled';
		pi.disableEffect = () => {
			pi.state = 'not-installed';
			return Promise.resolve({ kind: 'attempted' });
		};
		const controller = new DesktopAgentIntegrationManagementController(adapters);
		await controller.initialize();

		const result = await controller.disable('pi');

		expect(result.kind).toBe('completed');
		expect(pi.disableCount).toBe(1);
	});

	it('rejects concurrent operations instead of interleaving external writes', async () => {
		const adapters = createAdapters();
		const claude = adapter(adapters, 'claude-code');
		let release: (() => void) | undefined;
		claude.enableEffect = () =>
			new Promise((resolve) => {
				release = () => {
					claude.state = 'enabled';
					resolve({ kind: 'attempted' });
				};
			});
		const controller = new DesktopAgentIntegrationManagementController(adapters);
		await controller.initialize();

		const enabling = controller.enable('claude-code');
		await Promise.resolve();
		const refresh = await controller.refresh();
		release?.();

		expect(refresh).toMatchObject({ code: 'busy', kind: 'failed' });
		expect((await enabling).kind).toBe('completed');
	});

	it('preserves manual-only policy when inspection fails', async () => {
		const adapters = createAdapters();
		const omp = adapter(adapters, 'omp');
		omp.managementMode = 'manual-only';
		omp.inspectFailure = true;
		const controller = new DesktopAgentIntegrationManagementController(adapters);

		const snapshot = await controller.initialize();

		expect(snapshot.integrations[snapshot.integrations.length - 1]).toMatchObject({
			integrationId: 'omp',
			issue: 'inspection-failed',
			managementMode: 'manual-only',
			state: 'unknown',
		});
	});
});

class FakeAdapter implements AgentIntegrationAdapter {
	availability: DesktopAgentIntegrationAvailability = 'available';
	disableCount = 0;
	disableEffect: () => Promise<AgentIntegrationMutationAttempt> = () =>
		Promise.resolve({ kind: 'attempted' });
	enableCount = 0;
	enableEffect: () => Promise<AgentIntegrationMutationAttempt> = () =>
		Promise.resolve({ kind: 'attempted' });
	inspectFailure = false;
	readonly integrationId: DesktopAgentIntegrationId;
	managementMode: DesktopAgentIntegrationManagementMode = 'managed-local';
	state: DesktopAgentIntegrationState = 'not-installed';

	constructor(integrationId: DesktopAgentIntegrationId) {
		this.integrationId = integrationId;
	}

	disable(): Promise<AgentIntegrationMutationAttempt> {
		this.disableCount += 1;
		return this.disableEffect();
	}

	enable(): Promise<AgentIntegrationMutationAttempt> {
		this.enableCount += 1;
		return this.enableEffect();
	}

	inspect() {
		if (this.inspectFailure) return Promise.reject(new Error('inspection failed'));
		return Promise.resolve({
			availability: this.availability,
			integrationId: this.integrationId,
			issue: null,
			managementMode: this.managementMode,
			state: this.state,
		});
	}
}

function createAdapters(): FakeAdapter[] {
	return DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => new FakeAdapter(integrationId));
}

function adapter(
	adapters: readonly FakeAdapter[],
	integrationId: DesktopAgentIntegrationId,
): FakeAdapter {
	const result = adapters.find((candidate) => candidate.integrationId === integrationId);
	if (result === undefined) throw new Error('Missing fake adapter.');
	return result;
}
