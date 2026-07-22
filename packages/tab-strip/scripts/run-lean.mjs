import { createHash } from 'node:crypto';
import { chmod, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const toolsDirectory = path.join(repositoryRoot, '.tools');
const elanHome = path.join(toolsDirectory, 'elan-home');
const elanVersion = '4.2.1';
const elanBinary = path.join(elanHome, 'bin', 'elan');

const elanArtifacts = new Map([
	[
		'darwin-arm64',
		{
			name: 'elan-aarch64-apple-darwin.tar.gz',
			sha256: '3b3170eb6af7d89e28c7a98d25066a07efb41246e43d6333505dfa54069100c8',
		},
	],
	[
		'darwin-x64',
		{
			name: 'elan-x86_64-apple-darwin.tar.gz',
			sha256: '3bd377a1d767fabaad3b40d32bfa3d51b099ed40dbcd3c08ca66af2761ec1a70',
		},
	],
	[
		'linux-arm64',
		{
			name: 'elan-aarch64-unknown-linux-gnu.tar.gz',
			sha256: 'bb78726ace6a912c7122a389018bcd69d9122ce04659800101392f7db380d3b3',
		},
	],
	[
		'linux-x64',
		{
			name: 'elan-x86_64-unknown-linux-gnu.tar.gz',
			sha256: '4e717523217af592fa2d7b9c479410a31816c065d66ccbf0c2149337cfec0f5c',
		},
	],
]);

function run(command, args, options = {}) {
	const child = spawn(command, args, {
		cwd: repositoryRoot,
		stdio: 'inherit',
		...options,
	});
	return new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (signal) {
				reject(new Error(`${command} terminated by ${signal}`));
				return;
			}
			if (code !== 0) {
				reject(new Error(`${command} exited with status ${code}`));
				return;
			}
			resolve();
		});
	});
}

async function exists(filePath) {
	try {
		await readFile(filePath);
		return true;
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			return false;
		}
		throw error;
	}
}

async function provisionElan() {
	if (await exists(elanBinary)) {
		return;
	}

	const artifact = elanArtifacts.get(`${process.platform}-${process.arch}`);
	if (!artifact) {
		throw new Error(
			`No pinned elan ${elanVersion} artifact for ${process.platform}-${process.arch}`,
		);
	}

	await mkdir(toolsDirectory, { recursive: true });
	const archivePath = path.join(toolsDirectory, artifact.name);
	const extractDirectory = path.join(toolsDirectory, `elan-installer-${elanVersion}`);
	const downloadUrl = `https://github.com/leanprover/elan/releases/download/v${elanVersion}/${artifact.name}`;
	const response = await fetch(downloadUrl);
	if (!response.ok) {
		throw new Error(`Could not download ${downloadUrl}: HTTP ${response.status}`);
	}
	await writeFile(archivePath, new Uint8Array(await response.arrayBuffer()));

	const digest = createHash('sha256')
		.update(await readFile(archivePath))
		.digest('hex');
	if (digest !== artifact.sha256) {
		await rm(archivePath, { force: true });
		throw new Error(`Downloaded elan ${elanVersion} archive failed its pinned SHA-256 check`);
	}

	await rm(extractDirectory, { force: true, recursive: true });
	await mkdir(extractDirectory, { recursive: true });
	await run('tar', ['-xzf', archivePath, '-C', extractDirectory]);
	const installer = path.join(extractDirectory, 'elan-init');
	await chmod(installer, 0o755);
	await run(installer, ['-y', '--no-modify-path', '--default-toolchain', 'none'], {
		env: { ...process.env, ELAN_HOME: elanHome },
	});
	await rm(archivePath, { force: true });
	await rm(extractDirectory, { force: true, recursive: true });
}

await provisionElan();
await run(
	elanBinary,
	['run', '--install', 'leanprover/lean4:v4.30.0', 'lean', 'formal/lean/MoveContiguousBlock.lean'],
	{
		env: { ...process.env, ELAN_HOME: elanHome },
	},
);
