// Grok's installed inventory omits activation; its CLI writes user TOML lists.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const root = process.env.GROK_HOME;
const words = process.argv.slice(2);
const name = 'orchbee-agent-observation';
const statePath = path.join(root, 'grok-state.json');
const state = fs.existsSync(statePath)
	? JSON.parse(fs.readFileSync(statePath, 'utf8'))
	: { installed: false, source: null, version: null };
if (words.join(' ') === 'plugin list --json') {
	process.stdout.write(
		JSON.stringify(
			state.installed
				? [
						{
							status: state.status ?? 'installed',
							name,
							repo_key: 'owned-local',
							version: state.version,
							source: state.source,
							path: state.source,
							marketplace: null,
						},
					]
				: [],
		),
	);
} else {
	assert.equal(words[0], 'plugin');
	if (words[1] === 'install') {
		assert.equal(words.length, 4);
		assert.equal(words[3], '--trust');
		state.source = words[2];
		state.version = JSON.parse(
			fs.readFileSync(path.join(state.source, '.grok-plugin/plugin.json'), 'utf8'),
		).version;
		state.installed = true;
	} else if (words[1] === 'enable') {
		assert.deepEqual(words, ['plugin', 'enable', name]);
		assert.equal(state.installed, true);
	} else {
		assert.deepEqual(words, ['plugin', 'uninstall', name, '--confirm']);
		state.installed = false;
	}
	// Grok 1.0.77 keeps the activation entry when it uninstalls a plugin.
	if (state.installed) {
		fs.writeFileSync(
			path.join(root, 'config.toml'),
			`unrelated = 'retain'\n[plugins]\nenabled = ${JSON.stringify([name])}\ndisabled = []\n`,
		);
	}
	fs.appendFileSync(path.join(root, 'grok-calls.jsonl'), `${JSON.stringify(words)}\n`);
	fs.writeFileSync(statePath, JSON.stringify(state));
	process.stdout.write('{}');
}
