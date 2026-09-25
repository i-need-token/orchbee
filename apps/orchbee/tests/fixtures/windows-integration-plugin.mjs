// Distinct Claude and modern Copilot command contracts in an isolated profile.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root = process.env.CODEX_HOME;
const [provider, ...words] = process.argv.slice(2);
assert.ok(['claude', 'copilot'].includes(provider));
const statePath = path.join(root, `${provider}-state.json`);
const state = fs.existsSync(statePath)
	? JSON.parse(fs.readFileSync(statePath, 'utf8'))
	: { source: null, installed: false, enabled: false, version: null };
const id = 'orchbee-agent-observation@orchbee-local';
if (words.join(' ') === 'plugin list --json') {
	process.stdout.write(
		JSON.stringify(
			state.installed
				? [
						provider === 'claude'
							? { id, version: state.version, enabled: state.enabled }
							: {
									name: 'orchbee-agent-observation',
									version: state.version,
									enabled: state.enabled,
									source: state.sourceTag ?? 'live',
									marketplace: state.marketplace ?? 'orchbee-local',
									installedFrom: state.source,
								},
					]
				: [],
		),
	);
} else if (words.join(' ') === 'plugin marketplace list --json') {
	process.stdout.write(
		JSON.stringify(
			state.source === null
				? []
				: [
						provider === 'claude'
							? { name: 'orchbee-local', source: 'directory', path: state.source }
							: { name: 'orchbee-local', source: `Local: ${state.source}`, isDefault: false },
					],
		),
	);
} else {
	assert.equal(words[0], 'plugin');
	const scope = provider === 'claude' ? ['--scope', 'user'] : [];
	if (words[1] === 'marketplace') {
		assert.deepEqual(words.slice(4), scope);
		assert.equal(words.length, 4 + scope.length);
		if (words[2] === 'add') state.source = words[3];
		else {
			assert.equal(words[2], 'remove');
			assert.equal(words[3], 'orchbee-local');
			state.source = null;
		}
	} else {
		assert.deepEqual(words.slice(2), [id, ...scope]);
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
	fs.appendFileSync(path.join(root, `${provider}-calls.jsonl`), `${JSON.stringify(words)}\n`);
	fs.writeFileSync(statePath, JSON.stringify(state));
	process.stdout.write('{}');
}
