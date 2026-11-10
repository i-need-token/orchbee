// Deterministic local provider used by cli_windows_agent_integration.rs.
// It accepts only the Codex plugin argv exercised by the native adapter.
import fs from 'node:fs';
import path from 'node:path';
// Model a native provider's known-folder fallback, independent of USERPROFILE.
// Both choices are owned fixture directories; never consult the real user profile.
const root = process.env.CODEX_HOME || process.env.ORCHBEE_TEST_CODEX_KNOWN_FOLDER;
if (process.env.CODEX_HOME && !fs.existsSync(root)) process.exit(1);
fs.mkdirSync(root, { recursive: true });
const words = process.argv.slice(2);
if (process.env.ORCHBEE_TEST_CODEX_NO_EFFECT === '1') process.exit(0);
const statePath = path.join(root, 'fixture-state.json');
const state = fs.existsSync(statePath)
	? JSON.parse(fs.readFileSync(statePath, 'utf8'))
	: { source: null, plugin: false };
const id = 'orchbee-agent-observation@orchbee-local';
if (
	words.length === 5 &&
	words.slice(0, 3).join(' ') === 'plugin marketplace add' &&
	words[4] === '--json'
) {
	// Codex canonicalizes local sources with Rust's verbatim drive prefix.
	state.source = words[3].startsWith('\\\\?\\') ? words[3] : `\\\\?\\${words[3]}`;
} else if (
	words.length === 5 &&
	words.slice(0, 3).join(' ') === 'plugin marketplace remove' &&
	words[3] === 'orchbee-local' &&
	words[4] === '--json'
) {
	state.source = null;
} else if (
	words.length === 4 &&
	words[0] === 'plugin' &&
	words[2] === id &&
	words[3] === '--json'
) {
	// Like Codex 0.167, removal keeps the marketplace's now empty cache folder.
	const cache = path.join(root, 'plugins', 'cache', 'orchbee-local');
	if (words[1] === 'add') {
		state.plugin = true;
		fs.mkdirSync(path.join(cache, 'orchbee-agent-observation', 'installed'), { recursive: true });
	} else if (words[1] === 'remove') {
		state.plugin = false;
		fs.rmSync(path.join(cache, 'orchbee-agent-observation'), { recursive: true, force: true });
	} else process.exit(2);
} else {
	process.exit(2);
}
fs.appendFileSync(path.join(root, 'calls.jsonl'), `${JSON.stringify(words)}\n`);
fs.writeFileSync(statePath, JSON.stringify(state));
let config = "unrelated_user_setting = 'preserve me'\n";
if (state.source !== null)
	config += `[marketplaces.orchbee-local]\nsource_type = 'local'\nsource = ${JSON.stringify(state.source)}\n`;
if (state.plugin) config += `[plugins."${id}"]\nenabled = true\n`;
fs.writeFileSync(path.join(root, 'config.toml'), config);
process.stdout.write('{}\n');
