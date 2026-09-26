// Deterministic provider contract: profile-pinned JSON inventory and activation only.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
const profile = process.env.HERMES_HOME;
assert.ok(profile && path.isAbsolute(profile));
const args = process.argv.slice(2);
const directory = path.join(profile, 'plugins', 'orchbee-agent-observation');
const manifest = path.join(directory, 'plugin.yaml');
const selected = path.join(profile, 'fixture-plugin-enabled');
if (JSON.stringify(args) === JSON.stringify(['plugins', 'list', '--json'])) {
	const rows = fs.existsSync(manifest)
		? [
				{
					name: 'orchbee-agent-observation',
					version: JSON.parse(fs.readFileSync(manifest, 'utf8')).version,
					status: fs.existsSync(selected) ? 'enabled' : 'disabled',
					source: 'user',
					removed: null,
				},
			]
		: [];
	process.stdout.write(JSON.stringify(rows));
} else {
	assert.equal(args.length, 3);
	assert.equal(args[0], 'plugins');
	assert.ok(['enable', 'disable'].includes(args[1]));
	assert.equal(args[2], 'orchbee-agent-observation');
	assert.ok(fs.existsSync(manifest));
	const calls = path.join(profile, 'fixture-activation-calls');
	fs.appendFileSync(calls, JSON.stringify(args) + '\n');
	if (args[1] === 'enable') fs.writeFileSync(selected, 'enabled');
	else if (fs.existsSync(selected)) fs.unlinkSync(selected);
}
