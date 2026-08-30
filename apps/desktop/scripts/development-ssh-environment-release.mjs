import { spawn } from 'node:child_process';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import {
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	readdir,
	realpath,
	rename,
	rm,
	writeFile,
} from 'node:fs/promises';
import path from 'node:path';

import {
	SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
	SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	encodeSshEnvironmentReleaseTrustDescriptor,
} from '@orchbee/ssh-environment-protocol';

import {
	createSshEnvironmentRuntimeReleaseBuildArguments,
	createTerminalHostReleaseBuildArguments,
	finalizeSshEnvironmentRelease,
	prepareSshEnvironmentRelease,
} from '../../../scripts/ssh-environment-release.mjs';
import { readVerifiedSshEnvironmentRelease } from './ssh-environment-release-package.mjs';

const DEVELOPMENT_RELEASE_CACHE_SCHEMA_VERSION = 1;
const DEVELOPMENT_RELEASE_RECIPE = 'orchbee-development-ssh-environment-release-v1';
const DEVELOPMENT_RELEASE_MARKER_FILE_NAME = 'development-release-cache-v1.json';
const DEVELOPMENT_RELEASE_PUBLIC_KEY_FILE_NAME = 'development-signing-public-key.pem';
const DEVELOPMENT_VERSION_DIGEST_LENGTH = 32;
const MAXIMUM_TOOLCHAIN_OUTPUT_BYTES = 1024 * 1024;

const defaultDependencies = {
	buildComponent: buildDevelopmentReleaseComponent,
	finalizeRelease: finalizeSshEnvironmentRelease,
	generateKeyPair: () => generateKeyPairSync('ed25519'),
	prepareRelease: prepareSshEnvironmentRelease,
	readVerifiedRelease: readVerifiedSshEnvironmentRelease,
	resolveToolchainIdentity: resolveZigToolchainIdentity,
	signPayload: (payload, privateKey) => sign(null, payload, privateKey),
};

/**
 * Prepares one checkout-scoped signed cohort without persisting its development private key.
 * The returned files are immutable cache inputs for source startup or Dev packaging.
 */
export async function prepareDevelopmentSshEnvironmentRelease(options, dependencyOverrides = {}) {
	const dependencies = { ...defaultDependencies, ...dependencyOverrides };
	const configuredSourceRoot = requireAbsolutePath(options.sourceRoot, 'Source root');
	const outputRoot = requireAbsolutePath(options.outputRoot, 'Development release cache root');
	const sourceRoot = await realpath(configuredSourceRoot);
	const toolchainIdentity = await dependencies.resolveToolchainIdentity(sourceRoot);
	const identity = await resolveDevelopmentReleaseIdentity({ sourceRoot, toolchainIdentity });
	const targetDirectory = path.join(
		outputRoot,
		`${DEVELOPMENT_RELEASE_CACHE_SCHEMA_VERSION}-${identity.cohortInputDigest}`,
	);
	const existing = await pathKind(targetDirectory);
	if (existing !== 'missing') {
		if (existing !== 'directory') {
			throw new Error('Development SSH Environment cache target is not a directory.');
		}
		return verifyDevelopmentReleaseCache(targetDirectory, identity, dependencies);
	}

	await mkdir(outputRoot, { mode: 0o700, recursive: true });
	const candidateDirectory = await mkdtemp(path.join(outputRoot, '.development-release-'));
	try {
		const buildCacheDirectory = path.join(candidateDirectory, '.zig-build-cache');
		const preparedDirectory = path.join(candidateDirectory, '.prepared-release');
		const signatureFilePath = path.join(candidateDirectory, '.signature.bin');
		const releaseDirectoryPath = path.join(
			candidateDirectory,
			SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
		);
		const publicKeyFilePath = path.join(
			candidateDirectory,
			DEVELOPMENT_RELEASE_PUBLIC_KEY_FILE_NAME,
		);
		const trustDescriptorFilePath = path.join(
			candidateDirectory,
			SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
		);
		await dependencies.prepareRelease({
			buildComponent: (buildOptions) =>
				dependencies.buildComponent(
					buildOptions,
					path.join(buildCacheDirectory, buildOptions.role, buildOptions.platform),
				),
			outputDirectory: preparedDirectory,
			runtimeRoot: path.join(sourceRoot, 'apps', 'ssh-environment-runtime'),
			runtimeVersion: identity.runtimeVersion,
			terminalHostRoot: path.join(sourceRoot, 'apps', 'terminal-host'),
			terminalHostVersion: identity.terminalHostVersion,
		});
		const signingPayload = await readFile(
			path.join(preparedDirectory, SSH_ENVIRONMENT_RELEASE_SIGNING_PAYLOAD_FILE_NAME),
		);
		const keyPair = dependencies.generateKeyPair();
		const publicKey = keyPair.publicKey.export({ format: 'pem', type: 'spki' });
		await Promise.all([
			writeFile(publicKeyFilePath, publicKey, { mode: 0o400 }),
			writeFile(signatureFilePath, dependencies.signPayload(signingPayload, keyPair.privateKey), {
				mode: 0o600,
			}),
		]);
		await dependencies.finalizeRelease({
			outputDirectory: releaseDirectoryPath,
			preparedDirectory,
			publicKeyFilePath,
			signatureFilePath,
		});
		await Promise.all([
			rm(buildCacheDirectory, { force: true, recursive: true }),
			rm(preparedDirectory, { force: true, recursive: true }),
			rm(signatureFilePath, { force: true }),
		]);
		const verifiedRelease = await dependencies.readVerifiedRelease({
			publicKeyFilePath,
			releaseDirectory: releaseDirectoryPath,
		});
		await Promise.all([
			writeFile(
				trustDescriptorFilePath,
				encodeSshEnvironmentReleaseTrustDescriptor(verifiedRelease.trustDescriptor),
				{ mode: 0o400 },
			),
			writeFile(
				path.join(candidateDirectory, DEVELOPMENT_RELEASE_MARKER_FILE_NAME),
				encodeDevelopmentReleaseMarker(identity),
				{ mode: 0o400 },
			),
		]);
		await verifyDevelopmentReleaseCache(candidateDirectory, identity, dependencies);
		try {
			await rename(candidateDirectory, targetDirectory);
		} catch (error) {
			if (!isRenameCollision(error)) throw error;
			await rm(candidateDirectory, { force: true, recursive: true });
			return verifyDevelopmentReleaseCache(targetDirectory, identity, dependencies);
		}
		return verifyDevelopmentReleaseCache(targetDirectory, identity, dependencies);
	} finally {
		await rm(candidateDirectory, { force: true, recursive: true });
	}
}

async function buildDevelopmentReleaseComponent(options, cacheDirectory) {
	await mkdir(cacheDirectory, { mode: 0o700, recursive: true });
	const arguments_ =
		options.role === 'ssh-environment-runtime'
			? createSshEnvironmentRuntimeReleaseBuildArguments(options)
			: createTerminalHostReleaseBuildArguments(options);
	await runInheritedProgram(
		'zig',
		[...arguments_, '--cache-dir', cacheDirectory],
		options.componentRoot,
	);
}

async function resolveDevelopmentReleaseIdentity({ sourceRoot, toolchainIdentity }) {
	const sharedRecipeInputs = [
		path.join(sourceRoot, 'scripts', 'ssh-environment-release.mjs'),
		path.join(sourceRoot, 'apps', 'desktop', 'scripts', 'development-ssh-environment-release.mjs'),
	];
	const runtimeInputDigest = await digestComponentInputs(
		path.join(sourceRoot, 'apps', 'ssh-environment-runtime'),
		sharedRecipeInputs,
		sourceRoot,
		toolchainIdentity,
		'ssh-environment-runtime',
	);
	const terminalHostInputDigest = await digestComponentInputs(
		path.join(sourceRoot, 'apps', 'terminal-host'),
		sharedRecipeInputs,
		sourceRoot,
		toolchainIdentity,
		'terminal-host',
	);
	const cohortInputDigest = digestFields([
		DEVELOPMENT_RELEASE_RECIPE,
		sourceRoot,
		toolchainIdentity,
		runtimeInputDigest,
		terminalHostInputDigest,
	]);
	return {
		cohortInputDigest,
		runtimeInputDigest,
		runtimeVersion: developmentVersion(runtimeInputDigest),
		terminalHostInputDigest,
		terminalHostVersion: developmentVersion(terminalHostInputDigest),
	};
}

async function digestComponentInputs(
	componentRoot,
	sharedRecipeInputs,
	sourceRoot,
	toolchainIdentity,
	role,
) {
	const sourceInputs = await collectRegularFilePaths(path.join(componentRoot, 'src'));
	const inputPaths = [
		path.join(componentRoot, 'build.zig'),
		path.join(componentRoot, 'build.zig.zon'),
		...sourceInputs,
		...sharedRecipeInputs,
	].toSorted();
	const hash = createHash('sha256');
	updateDigestField(hash, DEVELOPMENT_RELEASE_RECIPE);
	updateDigestField(hash, sourceRoot);
	updateDigestField(hash, toolchainIdentity);
	updateDigestField(hash, role);
	for (const inputPath of inputPaths) {
		const metadata = await lstat(inputPath);
		if (!metadata.isFile()) {
			throw new Error(`Development release input is not a regular file: ${inputPath}`);
		}
		updateDigestField(hash, path.relative(sourceRoot, inputPath).split(path.sep).join('/'));
		updateDigestField(hash, await readFile(inputPath));
	}
	return hash.digest('hex');
}

async function collectRegularFilePaths(directory) {
	const entries = await readdir(directory, { withFileTypes: true });
	const paths = [];
	for (const entry of entries.toSorted((left, right) => left.name.localeCompare(right.name))) {
		const entryPath = path.join(directory, entry.name);
		if (entry.isDirectory()) paths.push(...(await collectRegularFilePaths(entryPath)));
		else if (entry.isFile()) paths.push(entryPath);
		else throw new Error(`Development release source input is not regular: ${entryPath}`);
	}
	return paths;
}

async function verifyDevelopmentReleaseCache(directory, expectedIdentity, dependencies) {
	await assertExactDirectory(directory, [
		DEVELOPMENT_RELEASE_MARKER_FILE_NAME,
		DEVELOPMENT_RELEASE_PUBLIC_KEY_FILE_NAME,
		SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
		SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	]);
	const markerBytes = await readFile(path.join(directory, DEVELOPMENT_RELEASE_MARKER_FILE_NAME));
	if (!markerBytes.equals(Buffer.from(encodeDevelopmentReleaseMarker(expectedIdentity)))) {
		throw new Error('Development SSH Environment cache marker does not match its inputs.');
	}
	const releaseDirectoryPath = path.join(directory, SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME);
	const publicKeyFilePath = path.join(directory, DEVELOPMENT_RELEASE_PUBLIC_KEY_FILE_NAME);
	const trustDescriptorFilePath = path.join(
		directory,
		SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
	);
	const verifiedRelease = await dependencies.readVerifiedRelease({
		publicKeyFilePath,
		releaseDirectory: releaseDirectoryPath,
	});
	const expectedTrustDescriptor = Buffer.from(
		encodeSshEnvironmentReleaseTrustDescriptor(verifiedRelease.trustDescriptor),
	);
	if (!(await readFile(trustDescriptorFilePath)).equals(expectedTrustDescriptor)) {
		throw new Error('Development SSH Environment trust descriptor is not canonical.');
	}
	return {
		cohortInputDigest: expectedIdentity.cohortInputDigest,
		publicKeyFilePath,
		releaseDirectoryPath,
		runtimeVersion: expectedIdentity.runtimeVersion,
		terminalHostVersion: expectedIdentity.terminalHostVersion,
		trustDescriptorFilePath,
	};
}

function encodeDevelopmentReleaseMarker(identity) {
	return `${JSON.stringify({
		cohortInputDigest: identity.cohortInputDigest,
		runtimeInputDigest: identity.runtimeInputDigest,
		runtimeVersion: identity.runtimeVersion,
		schemaVersion: DEVELOPMENT_RELEASE_CACHE_SCHEMA_VERSION,
		terminalHostInputDigest: identity.terminalHostInputDigest,
		terminalHostVersion: identity.terminalHostVersion,
	})}\n`;
}

function developmentVersion(inputDigest) {
	return `0.0.0-dev.${inputDigest.slice(0, DEVELOPMENT_VERSION_DIGEST_LENGTH)}`;
}

async function resolveZigToolchainIdentity(workingDirectory) {
	const output = await runProgram('zig', ['env'], workingDirectory);
	const zigExecutable = parseZigEnvironmentString(output, 'zig_exe');
	const libDirectory = parseZigEnvironmentString(output, 'lib_dir');
	const target = parseZigEnvironmentString(output, 'target');
	const version = parseZigEnvironmentString(output, 'version');
	return JSON.stringify({
		libDirectory: path.normalize(libDirectory),
		target,
		version,
		zigExecutable: path.normalize(zigExecutable),
	});
}

function parseZigEnvironmentString(output, field) {
	const match = new RegExp(`^\\s*\\.${field} = ("(?:[^"\\\\]|\\\\.)*"),?$`, 'mu').exec(output);
	if (match?.[1] === undefined) {
		throw new Error(`zig env omitted its ${field} identity field.`);
	}
	try {
		const value = JSON.parse(match[1]);
		if (typeof value === 'string' && value.length > 0) return value;
	} catch {
		// The bounded error below intentionally does not echo toolchain output or local paths.
	}
	throw new Error(`zig env returned an invalid ${field} identity field.`);
}

function runProgram(executable, arguments_, workingDirectory) {
	return new Promise((resolve, reject) => {
		const child = spawn(executable, arguments_, {
			cwd: workingDirectory,
			env: process.env,
			stdio: ['ignore', 'pipe', 'pipe'],
		});
		const stdout = [];
		const stderr = [];
		let outputBytes = 0;
		const collect = (chunks, bytes) => {
			outputBytes += bytes.byteLength;
			if (outputBytes > MAXIMUM_TOOLCHAIN_OUTPUT_BYTES) {
				child.kill();
				reject(new Error('Zig toolchain identity output exceeded its byte boundary.'));
				return;
			}
			chunks.push(bytes);
		};
		child.stdout.on('data', (bytes) => collect(stdout, bytes));
		child.stderr.on('data', (bytes) => collect(stderr, bytes));
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0) resolve(Buffer.concat(stdout).toString('utf8'));
			else {
				reject(
					new Error(
						`zig env failed with ${code ?? String(signal)}: ${Buffer.concat(stderr).toString('utf8').trim()}`,
					),
				);
			}
		});
	});
}

function runInheritedProgram(executable, arguments_, workingDirectory) {
	return new Promise((resolve, reject) => {
		const child = spawn(executable, arguments_, {
			cwd: workingDirectory,
			env: process.env,
			stdio: 'inherit',
		});
		child.once('error', reject);
		child.once('exit', (code, signal) => {
			if (code === 0 && signal === null) resolve();
			else {
				reject(
					new Error(`${executable} failed with code ${String(code)} and signal ${String(signal)}.`),
				);
			}
		});
	});
}

async function assertExactDirectory(directory, expectedEntries) {
	const metadata = await lstat(directory);
	if (!metadata.isDirectory()) {
		throw new Error('Development SSH Environment cache path is not a directory.');
	}
	const actual = (await readdir(directory)).toSorted();
	if (JSON.stringify(actual) !== JSON.stringify(expectedEntries.toSorted())) {
		throw new Error('Development SSH Environment cache has an unexpected layout.');
	}
}

async function pathKind(target) {
	try {
		const metadata = await lstat(target);
		return metadata.isDirectory() ? 'directory' : 'other';
	} catch (error) {
		if (isNodeError(error) && error.code === 'ENOENT') return 'missing';
		throw error;
	}
}

function digestFields(fields) {
	const hash = createHash('sha256');
	for (const field of fields) updateDigestField(hash, field);
	return hash.digest('hex');
}

function updateDigestField(hash, value) {
	const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
	hash.update(String(bytes.byteLength));
	hash.update(':');
	hash.update(bytes);
	hash.update(';');
}

function requireAbsolutePath(value, label) {
	if (typeof value !== 'string' || !path.isAbsolute(value)) {
		throw new Error(`${label} must be an absolute path.`);
	}
	return path.normalize(value);
}

function isRenameCollision(error) {
	return (
		isNodeError(error) &&
		(error.code === 'EEXIST' || error.code === 'ENOTEMPTY' || error.code === 'EPERM')
	);
}

function isNodeError(error) {
	return error instanceof Error && 'code' in error;
}
