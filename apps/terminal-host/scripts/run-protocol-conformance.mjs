import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const constantsRegistry = fileURLToPath(
	new URL('../../../schemas/terminal/v1/constants.json', import.meta.url),
);
const frameFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/terminal-frame.bin', import.meta.url),
);
const projectionFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/terminal-projection-buffer.bin', import.meta.url),
);
const packedProjectionConformanceFixture = fileURLToPath(
	new URL(
		'../../../schemas/terminal/v1/fixtures/packed-projection-conformance.json',
		import.meta.url,
	),
);
const envelopeFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/transport-envelope.bin', import.meta.url),
);
const inputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/write-input.bin', import.meta.url),
);
const keyInputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/key-input.bin', import.meta.url),
);
const textInputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/text-input.bin', import.meta.url),
);
const mouseInputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/mouse-input.bin', import.meta.url),
);
const focusInputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/focus-input.bin', import.meta.url),
);
const pasteInputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/paste-input.bin', import.meta.url),
);
const pasteAutoInputFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/paste-auto-input.bin', import.meta.url),
);
const terminalFailedFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/terminal-failed.json', import.meta.url),
);
const controlFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/control-conformance.json', import.meta.url),
);
const utf8BomFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/utf8-bom-conformance.json', import.meta.url),
);
const rowWindowFixture = fileURLToPath(
	new URL('../../../schemas/terminal/v1/fixtures/terminal-row-window.bin', import.meta.url),
);
const otherGenerationFixture = fileURLToPath(
	new URL(
		'../../../schemas/terminal/v1/fixtures/other-generation-conformance.json',
		import.meta.url,
	),
);

const child = spawn(
	'zig',
	[
		'build',
		'test-protocol',
		`-Dconstants-registry=${constantsRegistry}`,
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
		`-Dother-generation-fixture=${otherGenerationFixture}`,
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
