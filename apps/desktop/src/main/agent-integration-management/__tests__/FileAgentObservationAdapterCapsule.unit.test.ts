import { lstat, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it } from 'vitest';

import { FileAgentObservationAdapterCapsule } from '../FileAgentObservationAdapterCapsule';

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

	it('orders supplementary Unicode paths by UTF-16 code units in the shared digest vector', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		await writeFile(path.join(temporaryRoot, '\uE000'), 'B');
		await writeFile(path.join(temporaryRoot, '\u{10000}'), 'A');
		await expect(
			FileAgentObservationAdapterCapsule.computePayloadDigest(temporaryRoot),
		).resolves.toBe('34231f4c10b448ca3b14d2ef5668bdcf0302b3155c0b357d5d5f597b2d870460');
	});

	it('atomically retains a self-contained regular-file payload after the app source disappears', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const payloadSha256 = await writeSourceCapsule(sourcePath);
		const rootPath = path.join(
			temporaryRoot,
			'user-data',
			'agent-observation-adapters',
			payloadSha256,
		);
		const capsule = new FileAgentObservationAdapterCapsule({ rootPath, sourcePath });

		await expect(capsule.prepare()).resolves.toBe(true);
		expect((await lstat(rootPath)).isDirectory()).toBe(true);
		expect(
			await readFile(path.join(rootPath, 'bundle', 'scripts', 'forward-hook.mjs'), 'utf8'),
		).toBe('export {};\n');

		await rm(sourcePath, { recursive: true });
		await expect(capsule.prepare()).resolves.toBe(true);
		expect(await readFile(path.join(rootPath, 'capsule.json'), 'utf8')).toContain(payloadSha256);
	});

	it('rejects a source tree containing a symlink', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const payloadSha256 = await writeSourceCapsule(sourcePath);
		const rootPath = path.join(temporaryRoot, 'user-data', payloadSha256);
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
		const payloadSha256 = await writeSourceCapsule(actualSourcePath);
		const rootPath = path.join(temporaryRoot, 'user-data', payloadSha256);
		await symlink(actualSourcePath, sourcePath);

		await expect(
			new FileAgentObservationAdapterCapsule({ rootPath, sourcePath }).prepare(),
		).resolves.toBe(false);
		await expect(lstat(rootPath)).rejects.toMatchObject({ code: 'ENOENT' });
	});

	it('rejects non-current sources and installed payload drift', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const payloadSha256 = await writeSourceCapsule(sourcePath);
		const rootPath = path.join(temporaryRoot, 'user-data', payloadSha256);
		const capsule = new FileAgentObservationAdapterCapsule({ rootPath, sourcePath });
		for (const contractVersion of [2, 3, 4]) {
			await writeFile(
				path.join(sourcePath, 'capsule.json'),
				JSON.stringify({ contractVersion, payloadSha256 }),
			);
			expect(FileAgentObservationAdapterCapsule.readPayloadIdentity(sourcePath)).toBeNull();
			await expect(capsule.prepare()).resolves.toBe(false);
			await expect(lstat(rootPath)).rejects.toMatchObject({ code: 'ENOENT' });
		}
		await writeCapsuleManifest(sourcePath);
		await expect(capsule.prepare()).resolves.toBe(true);
		await writeFile(path.join(rootPath, 'unexpected.txt'), 'drift\n');

		await expect(capsule.isReady()).resolves.toBe(false);
	});

	it('detects a changed source payload even after its self-reported manifest is refreshed', async () => {
		temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-agent-capsule-test-'));
		const sourcePath = path.join(temporaryRoot, 'application-resources');
		const payloadSha256 = await writeSourceCapsule(sourcePath);
		const rootPath = path.join(temporaryRoot, 'user-data', payloadSha256);
		const capsule = new FileAgentObservationAdapterCapsule({ rootPath, sourcePath });
		await expect(capsule.prepare()).resolves.toBe(true);
		await writeFile(path.join(sourcePath, 'bundle', 'package.json'), '{"changed":true}\n');
		await writeCapsuleManifest(sourcePath);
		expect(FileAgentObservationAdapterCapsule.readPayloadIdentity(sourcePath)).not.toBe(
			payloadSha256,
		);

		await expect(capsule.isReady()).resolves.toBe(false);
		await expect(capsule.prepare()).resolves.toBe(false);
	});
});

async function writeSourceCapsule(rootPath: string): Promise<string> {
	const scriptsPath = path.join(rootPath, 'bundle', 'scripts');
	await mkdir(scriptsPath, { recursive: true });
	await Promise.all([
		writeFile(path.join(rootPath, 'bundle', 'package.json'), '{}\n'),
		writeFile(path.join(scriptsPath, 'forward-hook.mjs'), 'export {};\n'),
		writeFile(path.join(scriptsPath, 'hook-sanitizers.mjs'), 'export {};\n'),
		writeFile(path.join(scriptsPath, 'observation-transport.mjs'), 'export {};\n'),
	]);
	return writeCapsuleManifest(rootPath);
}

async function writeCapsuleManifest(rootPath: string): Promise<string> {
	const payloadSha256 = await FileAgentObservationAdapterCapsule.computePayloadDigest(rootPath);
	if (payloadSha256 === null) throw new Error('Fixture Capsule payload must be valid.');
	await writeFile(
		path.join(rootPath, 'capsule.json'),
		`${JSON.stringify({
			contractVersion: 1,
			payloadSha256,
		})}\n`,
	);
	return payloadSha256;
}
