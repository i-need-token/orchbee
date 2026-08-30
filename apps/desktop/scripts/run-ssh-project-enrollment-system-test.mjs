import { spawn } from 'node:child_process';
import { access, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const applicationRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(applicationRoot, '..', '..');

if (isSupportedPlatform()) {
	await runSshProjectEnrollmentSystemTest();
} else {
	console.log(
		'Skipping the SSH Project enrollment system witness: this runner requires macOS arm64 or glibc Linux x64.',
	);
}

async function runSshProjectEnrollmentSystemTest() {
	await runProgram('pnpm', ['--filter', '@orchbee/desktop', 'package:dev']);
	const packagedApplication = resolvePackagedApplication();
	await access(packagedApplication.executablePath);
	const componentVersions = await readPackagedComponentVersions(packagedApplication.manifestPath);
	const playwrightArguments = [
		'--filter',
		'@orchbee/desktop',
		'exec',
		'playwright',
		'test',
		'ssh-project-enrollment.spec.ts',
		'--config',
		'playwright.config.ts',
	];
	const testEnvironment = {
		ORCHBEE_SSH_PROJECT_ENROLLMENT_PACKAGE_EXECUTABLE: packagedApplication.executablePath,
		ORCHBEE_SSH_PROJECT_ENROLLMENT_RUNTIME_VERSION: componentVersions.runtimeVersion,
		ORCHBEE_SSH_PROJECT_ENROLLMENT_TERMINAL_HOST_VERSION: componentVersions.terminalHostVersion,
	};
	if (process.platform === 'linux' && process.env.DISPLAY === undefined) {
		await runProgram('/usr/bin/xvfb-run', ['-a', 'pnpm', ...playwrightArguments], testEnvironment);
	} else {
		await runProgram('pnpm', playwrightArguments, testEnvironment);
	}
}

function isSupportedPlatform() {
	return (
		(process.platform === 'darwin' && process.arch === 'arm64') ||
		(process.platform === 'linux' && process.arch === 'x64')
	);
}

function resolvePackagedApplication() {
	const applicationName = 'Orchbee Dev';
	const outputDirectory = path.join(
		applicationRoot,
		'out',
		'dev',
		`${applicationName}-${process.platform}-${process.arch}`,
	);
	if (process.platform === 'darwin') {
		const resourcesPath = path.join(
			outputDirectory,
			`${applicationName}.app`,
			'Contents',
			'Resources',
		);
		return {
			executablePath: path.join(
				outputDirectory,
				`${applicationName}.app`,
				'Contents',
				'MacOS',
				applicationName,
			),
			manifestPath: path.join(resourcesPath, 'ssh-environment-release-v2', 'manifest.json'),
		};
	}
	return {
		executablePath: path.join(outputDirectory, applicationName),
		manifestPath: path.join(
			outputDirectory,
			'resources',
			'ssh-environment-release-v2',
			'manifest.json',
		),
	};
}

async function readPackagedComponentVersions(manifestPath) {
	const { SshEnvironmentReleaseManifestSchema } = await import('@orchbee/ssh-environment-protocol');
	const manifest = SshEnvironmentReleaseManifestSchema.parse(
		JSON.parse(await readFile(manifestPath, 'utf8')),
	);
	const runtime = manifest.components.find(
		(component) => component.role === 'ssh-environment-runtime',
	);
	const terminalHost = manifest.components.find((component) => component.role === 'terminal-host');
	if (runtime === undefined || terminalHost === undefined) {
		throw new Error('The packaged Dev cohort omitted a required component.');
	}
	return { runtimeVersion: runtime.version, terminalHostVersion: terminalHost.version };
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
