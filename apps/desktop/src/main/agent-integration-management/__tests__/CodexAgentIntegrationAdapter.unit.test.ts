import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { CodexAgentIntegrationAdapter } from '../CodexAgentIntegrationAdapter';
import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';

const stableRoot = '/user-data/agent-observation-adapters/0.4.2';
const legacyRoot = '/Applications/Orchbee Local.app/Contents/Resources/agent-observation-adapters';

describe('CodexAgentIntegrationAdapter', () => {
	it('inspects a broken exact legacy marketplace without invoking the failing Codex inventory', async () => {
		const fixture = await createFixture(configuration(legacyRoot, true));
		try {
			await expect(fixture.adapter.inspect()).resolves.toMatchObject({
				availability: 'available',
				issue: null,
				state: 'update-required',
			});
			expect(fixture.runner.commands).toEqual([]);
		} finally {
			await fixture.dispose();
		}
	});

	it('migrates only after explicit enable through remove, retarget, and reinstall', async () => {
		const fixture = await createFixture(configuration(legacyRoot, true));
		try {
			await expect(fixture.adapter.enable()).resolves.toEqual({ kind: 'attempted' });
			expect(fixture.runner.commands).toEqual([
				['plugin', 'remove', 'orchbee-agent-observation@orchbee-local', '--json'],
				['plugin', 'marketplace', 'remove', 'orchbee-local', '--json'],
				['plugin', 'marketplace', 'add', path.resolve(stableRoot), '--json'],
				['plugin', 'add', 'orchbee-agent-observation@orchbee-local', '--json'],
			]);
		} finally {
			await fixture.dispose();
		}
	});

	it('removes both the plugin and stale owned marketplace on explicit disable', async () => {
		const fixture = await createFixture(configuration(legacyRoot, true));
		try {
			await expect(fixture.adapter.disable()).resolves.toEqual({ kind: 'attempted' });
			expect(fixture.runner.commands).toEqual([
				['plugin', 'remove', 'orchbee-agent-observation@orchbee-local', '--json'],
				['plugin', 'marketplace', 'remove', 'orchbee-local', '--json'],
			]);
		} finally {
			await fixture.dispose();
		}
	});

	it('distinguishes current, absent, disabled, and conflicting declarations', async () => {
		for (const [value, expected] of [
			[configuration(stableRoot, true), 'enabled'],
			[configuration(stableRoot, false), 'disabled'],
			['', 'not-installed'],
			[configuration('/third-party/marketplace', true), 'conflict'],
			[
				`${configuration(stableRoot, true)}\n[marketplaces.orchbee-local]\nsource_type = "local"\nsource = ${JSON.stringify(stableRoot)}\n`,
				'conflict',
			],
		] as const) {
			const fixture = await createFixture(value);
			try {
				await expect(fixture.adapter.inspect()).resolves.toMatchObject({ state: expected });
			} finally {
				await fixture.dispose();
			}
		}
	});
});

function configuration(resourceRoot: string, enabled: boolean): string {
	return `[marketplaces.orchbee-local]\nsource_type = "local"\nsource = ${JSON.stringify(resourceRoot)}\n\n[plugins."orchbee-agent-observation@orchbee-local"]\nenabled = ${enabled ? 'true' : 'false'}\n`;
}

async function createFixture(value: string): Promise<{
	readonly adapter: CodexAgentIntegrationAdapter;
	dispose(): Promise<void>;
	readonly runner: RecordingRunner;
}> {
	const homeDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-codex-adapter-'));
	await mkdir(path.join(homeDirectory, '.codex'), { recursive: true });
	await writeFile(path.join(homeDirectory, '.codex', 'config.toml'), value);
	const runner = new RecordingRunner();
	return {
		adapter: new CodexAgentIntegrationAdapter({
			commandRunner: runner,
			homeDirectory,
			legacyResourceRoots: [legacyRoot],
			managementMode: 'managed-local',
			resourceRoot: stableRoot,
		}),
		dispose: () => rm(homeDirectory, { force: true, recursive: true }),
		runner,
	};
}

class RecordingRunner implements AgentIntegrationCommandRunner {
	readonly commands: string[][] = [];

	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/codex');
	}

	run(
		_executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult> {
		this.commands.push([...commandArguments]);
		return Promise.resolve({ kind: 'completed', stdout: '{}' });
	}
}
