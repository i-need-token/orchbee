import { lstat, readFile, readdir } from 'node:fs/promises';
import { isAbsolute, join } from 'node:path';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
	SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
	SSH_RUNTIME_RELEASE_PLATFORMS,
	SshRuntimeReleaseTrustDescriptorSchema,
	encodeSshRuntimeReleaseTrustDescriptor,
	type SshEnvironmentPlatform,
} from '@orchbee/ssh-environment-protocol';

import {
	verifySshRuntimeArtifactAgainstCatalog,
	verifySshRuntimeReleaseCatalog,
	type VerifiedSshRuntimeArtifact,
	type VerifiedSshRuntimeReleaseCatalog,
} from './verify-ssh-runtime-artifact';

const MAXIMUM_TRUST_DESCRIPTOR_FILE_BYTES = 16 * 1024;
const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export type SshRuntimeReleaseCatalogLoadResult =
	| { readonly catalog: VerifiedSshRuntimeReleaseCatalog; readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export type SshRuntimeArtifactLoadResult =
	| { readonly artifact: VerifiedSshRuntimeArtifact; readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export interface SshRuntimeReleaseSource {
	loadArtifact(platform: SshEnvironmentPlatform): Promise<SshRuntimeArtifactLoadResult>;
	loadCatalog(): Promise<SshRuntimeReleaseCatalogLoadResult>;
}

export interface PackagedSshRuntimeReleaseSourceOptions {
	readonly releaseDirectoryPath: string;
	readonly trustDescriptorFilePath: string;
}

export class PackagedSshRuntimeReleaseSource implements SshRuntimeReleaseSource {
	readonly #artifactLoads = new Map<
		SshEnvironmentPlatform,
		Promise<SshRuntimeArtifactLoadResult>
	>();
	#catalogLoad: Promise<SshRuntimeReleaseCatalogLoadResult> | undefined;
	readonly #releaseDirectoryPath: string;
	readonly #trustDescriptorFilePath: string;

	constructor(options: PackagedSshRuntimeReleaseSourceOptions) {
		if (!isAbsolute(options.releaseDirectoryPath) || !isAbsolute(options.trustDescriptorFilePath)) {
			throw new Error('SSH Runtime release resource paths must be absolute.');
		}
		this.#releaseDirectoryPath = options.releaseDirectoryPath;
		this.#trustDescriptorFilePath = options.trustDescriptorFilePath;
	}

	loadArtifact(platform: SshEnvironmentPlatform): Promise<SshRuntimeArtifactLoadResult> {
		const existing = this.#artifactLoads.get(platform);
		if (existing !== undefined) return existing;
		const load = this.#loadArtifact(platform);
		this.#artifactLoads.set(platform, load);
		return load;
	}

	loadCatalog(): Promise<SshRuntimeReleaseCatalogLoadResult> {
		this.#catalogLoad ??= this.#loadCatalog();
		return this.#catalogLoad;
	}

	async #loadArtifact(platform: SshEnvironmentPlatform): Promise<SshRuntimeArtifactLoadResult> {
		try {
			const catalog = await this.loadCatalog();
			if (catalog.kind === 'unavailable') return catalog;
			const artifactBytes = await readRegularFile(
				join(this.#releaseDirectoryPath, SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[platform]),
				SSH_RUNTIME_RELEASE_MAXIMUM_ARTIFACT_BYTES,
			);
			const verification = verifySshRuntimeArtifactAgainstCatalog({
				artifactBytes,
				catalog: catalog.catalog,
				expectedPlatform: platform,
			});
			return verification.kind === 'verified'
				? { artifact: verification.artifact, kind: 'ready' }
				: { kind: 'unavailable' };
		} catch {
			return { kind: 'unavailable' };
		}
	}

	async #loadCatalog(): Promise<SshRuntimeReleaseCatalogLoadResult> {
		try {
			await assertReleaseLayout(this.#releaseDirectoryPath);
			const [manifestBytes, trustDescriptorBytes] = await Promise.all([
				readRegularFile(
					join(this.#releaseDirectoryPath, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
					SSH_RUNTIME_RELEASE_MAXIMUM_MANIFEST_BYTES,
				),
				readRegularFile(this.#trustDescriptorFilePath, MAXIMUM_TRUST_DESCRIPTOR_FILE_BYTES),
			]);
			const trustDescriptor = decodeCanonicalTrustDescriptor(trustDescriptorBytes);
			if (trustDescriptor === null) return { kind: 'unavailable' };
			const verification = verifySshRuntimeReleaseCatalog({
				manifestBytes,
				trustDescriptor,
			});
			if (verification.kind === 'failed') return { kind: 'unavailable' };
			for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
				const metadata = await lstat(
					join(this.#releaseDirectoryPath, SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS[platform]),
				);
				if (
					!metadata.isFile() ||
					metadata.size !== verification.catalog.artifacts[platform].sizeBytes
				) {
					return { kind: 'unavailable' };
				}
			}
			return { catalog: verification.catalog, kind: 'ready' };
		} catch {
			return { kind: 'unavailable' };
		}
	}
}

export class UnavailableSshRuntimeReleaseSource implements SshRuntimeReleaseSource {
	async loadArtifact(_platform: SshEnvironmentPlatform): Promise<SshRuntimeArtifactLoadResult> {
		return { kind: 'unavailable' };
	}

	async loadCatalog(): Promise<SshRuntimeReleaseCatalogLoadResult> {
		return { kind: 'unavailable' };
	}
}

async function assertReleaseLayout(root: string): Promise<void> {
	await assertExactDirectory(root, ['artifacts', SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME]);
	const artifactsRoot = join(root, 'artifacts');
	await assertExactDirectory(artifactsRoot, [...SSH_RUNTIME_RELEASE_PLATFORMS]);
	for (const platform of SSH_RUNTIME_RELEASE_PLATFORMS) {
		await assertExactDirectory(join(artifactsRoot, platform), [
			SSH_RUNTIME_RELEASE_ARTIFACT_FILE_NAME,
		]);
	}
}

async function assertExactDirectory(directory: string, expectedEntries: string[]): Promise<void> {
	const metadata = await lstat(directory);
	if (!metadata.isDirectory()) throw new Error('SSH Runtime release directory is invalid.');
	const actual = (await readdir(directory)).sort();
	const expected = expectedEntries.sort();
	if (JSON.stringify(actual) !== JSON.stringify(expected)) {
		throw new Error('SSH Runtime release layout is invalid.');
	}
}

async function readRegularFile(filePath: string, maximumBytes: number): Promise<Buffer> {
	const metadata = await lstat(filePath);
	if (!metadata.isFile() || metadata.size > maximumBytes) {
		throw new Error('SSH Runtime release resource is invalid.');
	}
	const bytes = await readFile(filePath);
	if (bytes.byteLength === 0 || bytes.byteLength > maximumBytes) {
		throw new Error('SSH Runtime release resource is invalid.');
	}
	return bytes;
}

function decodeCanonicalTrustDescriptor(bytes: Uint8Array) {
	let value: unknown;
	try {
		value = JSON.parse(utf8Decoder.decode(bytes));
	} catch {
		return null;
	}
	const parsed = SshRuntimeReleaseTrustDescriptorSchema.safeParse(value);
	if (!parsed.success) return null;
	return Buffer.from(encodeSshRuntimeReleaseTrustDescriptor(parsed.data)).equals(Buffer.from(bytes))
		? parsed.data
		: null;
}
