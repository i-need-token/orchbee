import { spawnSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import path from 'node:path';

import { checkoutDigest } from '../../../scripts/stage-local-runtime.mjs';
import { terminalHostBuildOptions } from '../../../scripts/stage-native-windows.mjs';

// Builds the Host into `zig-out` for this checkout's tests. A Host admits only Workers of its own
// environment, and a plain Cargo build is `ob-test-<checkout digest>` (crates/local-runtime/build.rs),
// so the Host receives that stem; a Host built without one matches no peer.
const hostRoot = path.resolve(import.meta.dirname, '..');
const root = path.resolve(hostRoot, '..', '..');
const environment = `ob-test-${checkoutDigest(realpathSync.native(root))}`;
const result = spawnSync(
	'zig',
	[
		'build',
		'-Doptimize=ReleaseSafe',
		...terminalHostBuildOptions(root, environment),
		...process.argv.slice(2),
	],
	{ cwd: hostRoot, stdio: 'inherit', windowsHide: true },
);
if (result.error !== undefined) throw result.error;
process.exitCode = result.status ?? 1;
