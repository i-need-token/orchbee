import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

// Knip follows a workspace package's entries to its built `dist`. Without that output every entry
// export of the package looks unused, which reads as hundreds of dead APIs instead of one missing
// build.
function unbuiltPackages() {
	const packagesRoot = join(root, 'packages');
	return readdirSync(packagesRoot, { withFileTypes: true })
		.filter((entry) => entry.isDirectory())
		.filter((entry) => {
			const manifestPath = join(packagesRoot, entry.name, 'package.json');
			if (!existsSync(manifestPath)) return false;
			const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
			return (
				typeof manifest.scripts?.build === 'string' &&
				!existsSync(join(packagesRoot, entry.name, 'dist'))
			);
		})
		.map((entry) => `packages/${entry.name}`);
}

// Knip does not discover an `.mjs` configuration by itself, so both runs name it.
function runKnip(mode) {
	process.stdout.write(`[dependencies] Knip, ${mode} graph\n`);
	const child = spawnSync(
		process.execPath,
		[
			join(root, 'node_modules/knip/bin/knip.js'),
			'--config',
			'knip.config.mjs',
			'--no-progress',
			'--treat-tag-hints-as-errors',
			...(mode === 'strict' ? ['--strict'] : []),
		],
		{ cwd: root, stdio: 'inherit', timeout: 180_000, killSignal: 'SIGKILL', windowsHide: true },
	);
	if (child.error) process.stderr.write(`[dependencies] ${String(child.error)}\n`);
	return child.status === 0;
}

function main() {
	const unbuilt = unbuiltPackages();
	if (unbuilt.length > 0) {
		process.stderr.write(
			`[dependencies] No build output for ${unbuilt.join(', ')}: run \`pnpm build:packages\` first.\n`,
		);
		return false;
	}
	// The strict graph repeats every finding of the complete graph, so it runs only after that passes.
	return runKnip('complete') && runKnip('strict');
}

if (!main()) process.exitCode = 1;
