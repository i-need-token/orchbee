import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const frameFixture = require.resolve('@orchbee/terminal-protocol/fixtures/terminal-frame-v2.bin');
const envelopeFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/transport-envelope-v1.bin');
const inputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/write-input-v1.bin');
const keyInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/key-input-v1_1.bin');
const textInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/text-input-v1_1.bin');
const mouseInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/mouse-input-v1_1.bin');
const focusInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/focus-input-v1_1.bin');
const pasteInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/paste-input-v1_1.bin');
const rowWindowFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-row-window-v1.bin');

const child = spawn(
	'zig',
	[
		'build',
		'test-protocol',
		`-Dframe-fixture=${frameFixture}`,
		`-Denvelope-fixture=${envelopeFixture}`,
		`-Dinput-fixture=${inputFixture}`,
		`-Dkey-input-fixture=${keyInputFixture}`,
		`-Dtext-input-fixture=${textInputFixture}`,
		`-Dmouse-input-fixture=${mouseInputFixture}`,
		`-Dfocus-input-fixture=${focusInputFixture}`,
		`-Dpaste-input-fixture=${pasteInputFixture}`,
		`-Drow-window-fixture=${rowWindowFixture}`,
	],
	{ stdio: 'inherit' },
);

await new Promise((resolve, reject) => {
	child.once('error', reject);
	child.once('exit', (code, signal) => {
		if (signal) {
			reject(new Error(`Terminal protocol conformance was terminated by ${signal}`));
			return;
		}
		if (code !== 0) {
			reject(new Error(`Terminal protocol conformance exited with status ${code}`));
			return;
		}
		resolve();
	});
});
