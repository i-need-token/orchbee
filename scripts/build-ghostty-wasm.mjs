import { createHash } from 'node:crypto';
import { access, copyFile, mkdtemp, readFile, rename, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const repositoryRoot = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const ghosttyRepository =
	process.env.GHOSTTY_REPOSITORY ?? 'https://github.com/ghostty-org/ghostty.git';
const ghosttyRevision = '91f66da24527fa02d92b5fd0b41cd020f553a64c';
const requiredZigVersion = '0.15.2';
const patchPath = path.join(repositoryRoot, 'packages/terminal/patches/ghostty-vt-decrqss.patch');
const artifactPath = path.join(repositoryRoot, 'packages/terminal/ghostty-vt.wasm');

function commandText(command, arguments_) {
	return [command, ...arguments_].map((value) => JSON.stringify(value)).join(' ');
}

function run(command, arguments_, options = {}) {
	process.stdout.write(`$ ${commandText(command, arguments_)}\n`);
	return new Promise((resolve, reject) => {
		const child = spawn(command, arguments_, {
			cwd: options.cwd ?? repositoryRoot,
			env: process.env,
			stdio: options.capture ? ['ignore', 'pipe', 'inherit'] : 'inherit',
		});
		let stdout = '';
		if (options.capture) {
			child.stdout.setEncoding('utf8');
			child.stdout.on('data', (chunk) => {
				stdout += chunk;
			});
		}
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0) {
				resolve(stdout.trim());
				return;
			}
			const reason = signal ? `signal ${signal}` : `exit code ${code ?? 'unknown'}`;
			reject(new Error(`${command} failed with ${reason}`));
		});
	});
}

async function resolveZig() {
	if (process.env.ZIG) return process.env.ZIG;
	const homebrewZig = '/opt/homebrew/opt/zig@0.15/bin/zig';
	try {
		await access(homebrewZig);
		return homebrewZig;
	} catch {
		return 'zig';
	}
}

async function main() {
	const zig = await resolveZig();
	const zigVersion = await run(zig, ['version'], { capture: true });
	if (zigVersion !== requiredZigVersion) {
		throw new Error(`Expected Zig ${requiredZigVersion}, received ${zigVersion}`);
	}

	const temporaryRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-ghostty-wasm-'));
	const sourceRoot = path.join(temporaryRoot, 'ghostty');
	const temporaryArtifact = `${artifactPath}.tmp-${process.pid}`;
	try {
		await run('git', [
			'clone',
			'--filter=blob:none',
			'--no-checkout',
			'--depth=1',
			ghosttyRepository,
			sourceRoot,
		]);
		await run('git', ['fetch', '--depth=1', 'origin', ghosttyRevision], { cwd: sourceRoot });
		await run('git', ['checkout', '--detach', ghosttyRevision], { cwd: sourceRoot });
		const checkedOutRevision = await run('git', ['rev-parse', 'HEAD'], {
			cwd: sourceRoot,
			capture: true,
		});
		if (checkedOutRevision !== ghosttyRevision) {
			throw new Error(`Expected Ghostty ${ghosttyRevision}, received ${checkedOutRevision}`);
		}

		await run('git', ['apply', '--unidiff-zero', '--check', patchPath], { cwd: sourceRoot });
		await run('git', ['apply', '--unidiff-zero', patchPath], { cwd: sourceRoot });
		await run(
			zig,
			['build', '-Demit-lib-vt=true', '-Dtarget=wasm32-freestanding', '-Doptimize=ReleaseSmall'],
			{ cwd: sourceRoot },
		);

		const builtArtifact = path.join(sourceRoot, 'zig-out/bin/ghostty-vt.wasm');
		const bytes = await readFile(builtArtifact);
		await WebAssembly.compile(bytes);
		await copyFile(builtArtifact, temporaryArtifact);
		await rename(temporaryArtifact, artifactPath);

		const digest = createHash('sha256').update(bytes).digest('hex');
		process.stdout.write(
			`Built ${path.relative(repositoryRoot, artifactPath)} (${bytes.length} bytes, sha256 ${digest})`,
		);
		process.stdout.write('\n');
	} finally {
		await rm(temporaryArtifact, { force: true });
		await rm(temporaryRoot, { recursive: true, force: true });
	}
}

main().catch((error) => {
	process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
	process.exitCode = 1;
});
