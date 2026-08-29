import { spawn } from 'node:child_process';
import { generateKeyPairSync, sign } from 'node:crypto';
import { access, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const RUNTIME_VERSION = '0.0.0-system-test';
const TERMINAL_HOST_VERSION = '0.0.0-system-test';
const applicationRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(applicationRoot, '..', '..');

requireSupportedPlatform();

const fixtureRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-ssh-enrollment-release-'));
try {
	await runProgram('pnpm', ['--filter', '@orchbee/ssh-environment-protocol', 'build']);
	const { finalizeSshEnvironmentRelease, prepareSshEnvironmentRelease } =
		await import('../../../scripts/ssh-environment-release.mjs');
	const preparedDirectory = path.join(fixtureRoot, 'prepared-release');
	const finalizedDirectory = path.join(fixtureRoot, 'final-release');
	const publicKeyFilePath = path.join(fixtureRoot, 'release-public-key.pem');
	const signatureFilePath = path.join(fixtureRoot, 'release-signature.bin');
	await prepareSshEnvironmentRelease({
		outputDirectory: preparedDirectory,
		runtimeRoot: path.join(repositoryRoot, 'apps', 'ssh-environment-runtime'),
		runtimeVersion: RUNTIME_VERSION,
		terminalHostRoot: path.join(repositoryRoot, 'apps', 'terminal-host'),
		terminalHostVersion: TERMINAL_HOST_VERSION,
	});
	const signingPayload = await readFile(
		path.join(preparedDirectory, 'manifest-signing-payload.json'),
	);
	const keyPair = generateKeyPairSync('ed25519');
	await Promise.all([
		writeFile(publicKeyFilePath, keyPair.publicKey.export({ format: 'pem', type: 'spki' }), {
			mode: 0o600,
		}),
		writeFile(signatureFilePath, sign(null, signingPayload, keyPair.privateKey), { mode: 0o600 }),
	]);
	await finalizeSshEnvironmentRelease({
		outputDirectory: finalizedDirectory,
		preparedDirectory,
		publicKeyFilePath,
		signatureFilePath,
	});
	await runProgram('pnpm', ['--filter', '@orchbee/desktop', 'package:release'], {
		ORCHBEE_SSH_ENVIRONMENT_RELEASE_DIRECTORY: finalizedDirectory,
		ORCHBEE_SSH_ENVIRONMENT_RELEASE_PUBLIC_KEY: publicKeyFilePath,
	});
	const packagedExecutable = resolvePackagedExecutable();
	await access(packagedExecutable);
	await runProgram(
		'pnpm',
		[
			'--filter',
			'@orchbee/desktop',
			'exec',
			'playwright',
			'test',
			'ssh-project-enrollment.spec.ts',
			'--config',
			'playwright.config.ts',
		],
		{
			ORCHBEE_SSH_PROJECT_ENROLLMENT_PACKAGE_EXECUTABLE: packagedExecutable,
			ORCHBEE_SSH_PROJECT_ENROLLMENT_RUNTIME_VERSION: RUNTIME_VERSION,
			ORCHBEE_SSH_PROJECT_ENROLLMENT_TERMINAL_HOST_VERSION: TERMINAL_HOST_VERSION,
		},
	);
} finally {
	await rm(fixtureRoot, { force: true, recursive: true });
}

function requireSupportedPlatform() {
	if (
		(process.platform === 'darwin' && process.arch === 'arm64') ||
		(process.platform === 'linux' && process.arch === 'x64')
	) {
		return;
	}
	throw new Error(
		'SSH Project enrollment system evidence requires macOS arm64 or glibc Linux x64.',
	);
}

function resolvePackagedExecutable() {
	const outputDirectory = path.join(
		applicationRoot,
		'out',
		`Orchbee-${process.platform}-${process.arch}`,
	);
	return process.platform === 'darwin'
		? path.join(outputDirectory, 'Orchbee.app', 'Contents', 'MacOS', 'Orchbee')
		: path.join(outputDirectory, 'Orchbee');
}

function runProgram(executable, arguments_, environment = {}) {
	return new Promise((resolve, reject) => {
		const child = spawn(executable, arguments_, {
			cwd: repositoryRoot,
			env: { ...process.env, ...environment },
			stdio: 'inherit',
		});
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0) resolve();
			else {
				reject(
					new Error(`${executable} ${arguments_.join(' ')} failed with ${code ?? String(signal)}.`),
				);
			}
		});
	});
}
