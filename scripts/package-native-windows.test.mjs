import assert from 'node:assert/strict';
import {
	mkdtempSync,
	realpathSync,
	rmSync,
	writeFileSync,
	readFileSync,
	existsSync,
	unlinkSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { componentEntries, packageWindows } from './package-native-windows.mjs';

test('release packaging rejects incomplete cohorts and preserves existing feeds', (t) => {
	const parent = realpathSync(tmpdir());
	const root = mkdtempSync(path.join(parent, 'orchbee-release-test-'));
	t.after(() => {
		const actual = realpathSync(root);
		assert.equal(path.dirname(actual), parent);
		assert.ok(path.basename(actual).startsWith('orchbee-release-test-'));
		rmSync(actual, { recursive: true });
	});
	assert.throws(() => packageWindows(root, 'nightly', '0.1.0', 'test'));
	for (const version of ['0.1.0+build.1', '01.0.0', '0.1', '0.1.0-01', 'a'.repeat(65)])
		assert.throws(() => packageWindows(root, 'stable', version, 'test'), /SemVer/u, version);
	assert.throws(() => packageWindows(root, 'stable', '0.1.0', 'test'));
	assert.equal(existsSync(path.join(root, 'release.json')), false);
	writeFileSync(path.join(root, 'cohort.json'), '{"schemaVersion":1,"artifacts":{}}');
	assert.throws(() => packageWindows(root, 'stable', '0.1.0', 'test'), /cohort manifest/u);
	for (const name of ['release.json', 'release.json.sig']) {
		writeFileSync(path.join(root, name), 'existing immutable release');
		assert.throws(() => packageWindows(root, 'stable', '0.1.0', 'test'), /already exists/u);
		assert.equal(readFileSync(path.join(root, name), 'utf8'), 'existing immutable release');
		unlinkSync(path.join(root, name));
	}
});

test('every generation registry contract belongs to a component that lists only cohort files', () => {
	const files = [
		'agent-observation-adapters/capsule.json',
		'cohort.json',
		'conpty/conpty.dll',
		'orchbee-server.exe',
		'orchbee-terminal-host.exe',
		'orchbee-worker.exe',
		'orchbee.exe',
	].map((file) => ({ path: file }));
	const entries = componentEntries(files, '0.1.0');
	const registry = JSON.parse(
		readFileSync(new URL('../schemas/connection/v1/connections.json', import.meta.url), 'utf8'),
	);
	const contracts = new Map(
		['negotiatedConnections', 'frameFormats', 'deliveries'].flatMap((group) =>
			Object.entries(registry[group]).map(([name, entry]) => [name, entry.generations]),
		),
	);
	for (const [name, generations] of contracts) {
		const users = entries.filter((entry) => name in entry.connections);
		assert.ok(users.length > 0, name);
		for (const entry of users)
			assert.deepEqual(entry.connections[name].generations, {
				lowest: generations.lowest,
				highest: generations.highest,
			});
	}
	for (const entry of entries) {
		assert.equal(entry.version, '0.1.0');
		assert.ok(entry.files.length > 0 && !entry.files.includes('cohort.json'), entry.id);
	}
	assert.throws(
		() =>
			componentEntries(
				files.filter((file) => file.path !== 'orchbee-worker.exe'),
				'0.1.0',
			),
		/worker has no files/u,
	);
});
