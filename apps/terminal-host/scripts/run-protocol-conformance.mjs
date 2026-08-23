import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const frameFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin');
const projectionFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-projection-buffer-v0.0.2.bin');
const envelopeFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/transport-envelope-v0.0.19.bin');
const inputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/write-input-v0.0.19.bin');
const keyInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/key-input-v0.0.19.bin');
const textInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/text-input-v0.0.19.bin');
const mouseInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/mouse-input-v0.0.19.bin');
const focusInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/focus-input-v0.0.19.bin');
const pasteInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/paste-input-v0.0.19.bin');
const terminalFailedFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-failed-v0.0.19.json');
const controlFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/control-conformance-v0.0.19.json');
const utf8BomFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/utf8-bom-conformance-v0.0.19.json');
const rowWindowFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-row-window-v0.0.2.bin');

const child = spawn(
	'zig',
	[
		'build',
		'test-protocol',
		`-Dframe-fixture=${frameFixture}`,
		`-Dprojection-fixture=${projectionFixture}`,
		`-Denvelope-fixture=${envelopeFixture}`,
		`-Dinput-fixture=${inputFixture}`,
		`-Dkey-input-fixture=${keyInputFixture}`,
		`-Dtext-input-fixture=${textInputFixture}`,
		`-Dmouse-input-fixture=${mouseInputFixture}`,
		`-Dfocus-input-fixture=${focusInputFixture}`,
		`-Dpaste-input-fixture=${pasteInputFixture}`,
		`-Dterminal-failed-fixture=${terminalFailedFixture}`,
		`-Dcontrol-fixture=${controlFixture}`,
		`-Dutf8-bom-fixture=${utf8BomFixture}`,
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
