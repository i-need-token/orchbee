import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const frameFixture = require.resolve('@orchbee/terminal-protocol/fixtures/terminal-frame.bin');
const projectionFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-projection-buffer.bin');
const packedProjectionConformanceFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/packed-projection-conformance.json');
const envelopeFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/transport-envelope.bin');
const inputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/write-input.bin');
const keyInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/key-input.bin');
const textInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/text-input.bin');
const mouseInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/mouse-input.bin');
const focusInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/focus-input.bin');
const pasteInputFixture = require.resolve('@orchbee/terminal-protocol/fixtures/paste-input.bin');
const pasteAutoInputFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/paste-auto-input.bin');
const terminalFailedFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-failed.json');
const controlFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/control-conformance.json');
const utf8BomFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/utf8-bom-conformance.json');
const rowWindowFixture =
	require.resolve('@orchbee/terminal-protocol/fixtures/terminal-row-window.bin');

const child = spawn(
	'zig',
	[
		'build',
		'test-protocol',
		`-Dframe-fixture=${frameFixture}`,
		`-Dprojection-fixture=${projectionFixture}`,
		`-Dpacked-projection-conformance-fixture=${packedProjectionConformanceFixture}`,
		`-Denvelope-fixture=${envelopeFixture}`,
		`-Dinput-fixture=${inputFixture}`,
		`-Dkey-input-fixture=${keyInputFixture}`,
		`-Dtext-input-fixture=${textInputFixture}`,
		`-Dmouse-input-fixture=${mouseInputFixture}`,
		`-Dfocus-input-fixture=${focusInputFixture}`,
		`-Dpaste-input-fixture=${pasteInputFixture}`,
		`-Dpaste-auto-input-fixture=${pasteAutoInputFixture}`,
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
