import { spawn } from 'node:child_process';
import { copyFile, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const hostRoot = fileURLToPath(new URL('../', import.meta.url));
const fixtureNames = ['ghostty-surface-v0.0.1.bin', 'ghostty-alternate-surface-v0.0.1.bin'];
const write = process.argv.slice(2).includes('--write');
const temporaryDirectory = await mkdtemp(join(tmpdir(), 'orchbee-ghostty-surface-'));
const fixtures = fixtureNames.map((name) => ({
	generated: join(temporaryDirectory, name),
	name,
	tracked: fileURLToPath(
		new URL(`../../../packages/terminal-protocol/fixtures/${name}`, import.meta.url),
	),
}));

try {
	const child = spawn(
		'zig',
		[
			'build',
			'generate-surface-fixture',
			'-Doptimize=ReleaseSafe',
			'--',
			...fixtures.map(({ generated }) => generated),
		],
		{ cwd: hostRoot, stdio: 'inherit' },
	);
	await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (signal) {
				reject(new Error(`Ghostty surface fixture generation was terminated by ${signal}`));
				return;
			}
			if (code !== 0) {
				reject(new Error(`Ghostty surface fixture generation exited with status ${code}`));
				return;
			}
			resolve();
		});
	});

	if (write) {
		await Promise.all(fixtures.map(({ generated, tracked }) => copyFile(generated, tracked)));
	} else {
		const comparisons = await Promise.all(
			fixtures.map(async ({ generated, name, tracked }) => {
				const [generatedBytes, trackedBytes] = await Promise.all([
					readFile(generated),
					readFile(tracked),
				]);
				return { generatedBytes, name, trackedBytes };
			}),
		);
		for (const { generatedBytes, name, trackedBytes } of comparisons) {
			if (!generatedBytes.equals(trackedBytes)) {
				throw new Error(
					`${name} is stale; run pnpm --filter @orchbee/terminal-host surface-fixtures:generate`,
				);
			}
		}
	}
} finally {
	await rm(temporaryDirectory, { force: true, recursive: true });
}
