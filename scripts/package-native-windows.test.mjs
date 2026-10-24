import assert from 'node:assert/strict';
import {
	mkdtempSync,
	realpathSync,
	rmSync,
	writeFileSync,
	readFileSync,
	existsSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { packageWindows } from './package-native-windows.mjs';

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
	assert.throws(() => packageWindows(root, 'stable', '0.1.0', 'test'));
	assert.equal(existsSync(path.join(root, 'release.json')), false);
	writeFileSync(path.join(root, 'cohort.json'), '{"schemaVersion":1,"artifacts":{}}');
	assert.throws(() => packageWindows(root, 'stable', '0.1.0', 'test'), /cohort manifest/u);
	writeFileSync(path.join(root, 'release.json'), 'existing immutable release');
	assert.throws(() => packageWindows(root, 'stable', '0.1.0', 'test'), /already exists/u);
	assert.equal(readFileSync(path.join(root, 'release.json'), 'utf8'), 'existing immutable release');
});
