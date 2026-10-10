import { resolve } from 'node:path';

import { checkVerbose, runChecks } from '../../../scripts/check-output.mjs';

const targets = ['x86_64-linux-gnu', 'x86_64-freebsd', 'aarch64-macos', 'x86_64-windows-gnu'];

// Each target builds several large Ghostty test roots. Serial targets and bounded Zig
// jobs prevent four independent build schedulers from exhausting the same memory.
process.exitCode = await runChecks({
	name: 'terminal-host:test:cross',
	steps: targets.map((target) => ({
		name: target,
		command: 'zig',
		args: ['build', 'test-compile', '-j2', `-Dtarget=${target}`, '-Doptimize=ReleaseSafe'],
	})),
	cwd: resolve(import.meta.dirname, '..'),
	verbose: checkVerbose(process.argv.slice(2)),
});
