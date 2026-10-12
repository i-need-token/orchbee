import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createHash } from 'node:crypto';
import { chmod, copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(desktopRoot, '..', '..');

/** Materialize the exact portable settings adapter consumed by development and packaging. */
export async function buildSettingsBridge() {
	const { stdout } = await promisify(execFile)('rustc', ['-vV'], {
		cwd: repositoryRoot,
		windowsHide: true,
		timeout: 10_000,
		maxBuffer: 16 * 1024,
	});
	const target = /^host: ([a-zA-Z0-9_-]+)$/mu.exec(stdout)?.[1];
	if (target === undefined) throw new Error('The native Rust host target is unavailable.');
	await new Promise((resolve, reject) => {
		const child = spawn(
			'cargo',
			[
				'build',
				'--locked',
				'--release',
				'-p',
				'orchbee-settings',
				'--target',
				target,
				'--target-dir',
				path.join(repositoryRoot, 'target'),
			],
			{
				cwd: repositoryRoot,
				stdio: 'inherit',
				windowsHide: true,
			},
		);
		child.once('error', reject);
		child.once('close', (code) => {
			if (code === 0) resolve();
			else reject(new Error('Settings bridge build failed.'));
		});
	});
	const executable = process.platform === 'win32' ? 'orchbee-settings.exe' : 'orchbee-settings';
	const source = path.join(repositoryRoot, 'target', target, 'release', executable);
	const directory = path.join(desktopRoot, 'out', 'settings-bridge');
	await mkdir(directory, { recursive: true });
	const destination = path.join(directory, executable);
	await copyFile(source, destination);
	if (process.platform !== 'win32') await chmod(destination, 0o700);
	const sha256 = createHash('sha256')
		.update(await readFile(destination))
		.digest('hex');
	await writeFile(
		path.join(directory, 'manifest.json'),
		`${JSON.stringify({ version: 1, executable, sha256 })}\n`,
	);
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	await buildSettingsBridge();
}
