import { lstat, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import {
	AGENT_OBSERVATION_CAPSULE_VERSION,
	FileAgentObservationAdapterCapsule,
} from '../file-agent-observation-adapter-capsule';

describe('FileAgentObservationAdapterCapsule', () => {
	let temporaryRoot: string | undefined;

	afterEach(async () => {
		if (temporaryRoot !== undefined) await rm(temporaryRoot, { force: true, recursive: true });
		temporaryRoot = undefined;
	});

	it('accepts the checked-in source Capsule and its exact payload digest', async () => {
		const sourcePath = path.resolve(process.cwd(), 'resources/agent-observation-adapters');
		const capsule = new FileAgentObservationAdapterCapsule({
			rootPath: sourcePath,
			sourcePath,
		});

		await expect(capsule.isReady()).resolves.toBe(true);
	});

	it('atomically retains a self-contained regular-file payload after the app source disappears', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const rootPath = path.join(
			temporaryRoot,
			'user-data',
			'agent-observation-adapters',
			AGENT_OBSERVATION_CAPSULE_VERSION,
		);
		await writeSourceCapsule(sourcePath);
		const capsule = new FileAgentObservationAdapterCapsule({ rootPath, sourcePath });

		await expect(capsule.prepare()).resolves.toBe(true);
		expect((await lstat(rootPath)).isDirectory()).toBe(true);
		expect(
			await readFile(path.join(rootPath, 'bundle', 'scripts', 'forward-hook.mjs'), 'utf8'),
		).toBe('export {};\n');

		await rm(sourcePath, { recursive: true });
		await expect(capsule.prepare()).resolves.toBe(true);
		expect(await readFile(path.join(rootPath, 'capsule.json'), 'utf8')).toContain(
			AGENT_OBSERVATION_CAPSULE_VERSION,
		);
	});

	it('rejects a source tree containing a symlink', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const rootPath = path.join(temporaryRoot, 'user-data', AGENT_OBSERVATION_CAPSULE_VERSION);
		await writeSourceCapsule(sourcePath);
		await symlink('/private/unowned', path.join(sourcePath, 'unowned-link'));

		await expect(
			new FileAgentObservationAdapterCapsule({ rootPath, sourcePath }).prepare(),
		).resolves.toBe(false);
		await expect(lstat(rootPath)).rejects.toMatchObject({ code: 'ENOENT' });
	});

	it('rejects a source root that is itself a symlink', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const actualSourcePath = path.join(temporaryRoot, 'actual-application-resources');
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const rootPath = path.join(temporaryRoot, 'user-data', AGENT_OBSERVATION_CAPSULE_VERSION);
		await writeSourceCapsule(actualSourcePath);
		await symlink(actualSourcePath, sourcePath);

		await expect(
			new FileAgentObservationAdapterCapsule({ rootPath, sourcePath }).prepare(),
		).resolves.toBe(false);
		await expect(lstat(rootPath)).rejects.toMatchObject({ code: 'ENOENT' });
	});

	it('rejects payload drift and extra entries against the embedded digest', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const rootPath = path.join(temporaryRoot, 'user-data', AGENT_OBSERVATION_CAPSULE_VERSION);
		await writeSourceCapsule(sourcePath);
		const capsule = new FileAgentObservationAdapterCapsule({ rootPath, sourcePath });
		await expect(capsule.prepare()).resolves.toBe(true);
		await writeFile(path.join(rootPath, 'unexpected.txt'), 'drift\n');

		await expect(capsule.isReady()).resolves.toBe(false);
	});

	it('does not mistake a different same-version source payload for the published Capsule', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const rootPath = path.join(temporaryRoot, 'user-data', AGENT_OBSERVATION_CAPSULE_VERSION);
		await writeSourceCapsule(sourcePath);
		const capsule = new FileAgentObservationAdapterCapsule({ rootPath, sourcePath });
		await expect(capsule.prepare()).resolves.toBe(true);
		await writeFile(path.join(sourcePath, 'bundle', 'package.json'), '{"changed":true}\n');
		await writeCapsuleManifest(sourcePath);

		await expect(capsule.isReady()).resolves.toBe(false);
		await expect(capsule.prepare()).resolves.toBe(false);
	});
});

async function writeSourceCapsule(rootPath: string): Promise<void> {
	const scriptsPath = path.join(rootPath, 'bundle', 'scripts');
	await mkdir(scriptsPath, { recursive: true });
	await Promise.all([
		writeFile(path.join(rootPath, 'bundle', 'package.json'), '{}\n'),
		writeFile(path.join(scriptsPath, 'forward-hook.mjs'), 'export {};\n'),
		writeFile(path.join(scriptsPath, 'hook-sanitizers.mjs'), 'export {};\n'),
		writeFile(path.join(scriptsPath, 'observation-transport.mjs'), 'export {};\n'),
	]);
	await writeCapsuleManifest(rootPath);
}

async function writeCapsuleManifest(rootPath: string): Promise<void> {
	const payloadSha256 = await FileAgentObservationAdapterCapsule.computePayloadDigest(rootPath);
	if (payloadSha256 === null) throw new Error('Fixture Capsule payload must be valid.');
	await writeFile(
		path.join(rootPath, 'capsule.json'),
		`${JSON.stringify({
			contractVersion: 3,
			payloadSha256,
			payloadVersion: AGENT_OBSERVATION_CAPSULE_VERSION,
		})}\n`,
	);
}
