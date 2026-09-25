// Isolated Claude plugin command contract for Windows onboarding witnesses.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root = process.env.CODEX_HOME;
const words = process.argv.slice(2);
const statePath = path.join(root, 'claude-state.json');
const state = fs.existsSync(statePath)
	? JSON.parse(fs.readFileSync(statePath, 'utf8'))
	: { source: null, installed: false, enabled: false, version: null };
const id = 'orchbee-agent-observation@orchbee-local';
if (words.join(' ') === 'plugin list --json') {
	process.stdout.write(
		JSON.stringify(state.installed ? [{ id, version: state.version, enabled: state.enabled }] : []),
	);
} else if (words.join(' ') === 'plugin marketplace list --json') {
	process.stdout.write(
		JSON.stringify(
			state.source === null
				? []
				: [{ name: 'orchbee-local', source: 'directory', path: state.source }],
		),
	);
} else {
	assert.equal(words[0], 'plugin');
	if (words[1] === 'marketplace') {
		assert.deepEqual(words.slice(-2), ['--scope', 'user']);
		assert.equal(words.length, 6);
		if (words[2] === 'add') state.source = words[3];
		else {
			assert.equal(words[2], 'remove');
			assert.equal(words[3], 'orchbee-local');
			state.source = null;
		}
	} else {
		assert.deepEqual(words.slice(2), [id, '--scope', 'user']);
		assert.ok(['install', 'uninstall', 'enable', 'disable'].includes(words[1]));
		if (words[1] === 'install') {
			assert.ok(state.source);
			state.version = JSON.parse(
				fs.readFileSync(path.join(state.source, 'capsule.json'), 'utf8'),
			).payloadVersion;
			state.installed = true;
			state.enabled = true;
		} else if (words[1] === 'uninstall') {
			state.installed = false;
			state.enabled = false;
		} else state.enabled = words[1] === 'enable';
	}
	fs.appendFileSync(path.join(root, 'claude-calls.jsonl'), `${JSON.stringify(words)}\n`);
	fs.writeFileSync(statePath, JSON.stringify(state));
	process.stdout.write('{}');
}
