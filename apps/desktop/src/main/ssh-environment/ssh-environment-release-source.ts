import { lstat, readFile, readdir } from 'node:fs/promises';
import { isAbsolute, join } from 'node:path';

import {
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES,
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES,
	SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_MAXIMUM_ARTIFACT_BYTES,
	SSH_ENVIRONMENT_RELEASE_MAXIMUM_MANIFEST_BYTES,
	SSH_ENVIRONMENT_RELEASE_PLATFORMS,
	SshEnvironmentReleaseTrustDescriptorSchema,
	encodeSshEnvironmentReleaseTrustDescriptor,
	type SshEnvironmentPlatform,
	type SshEnvironmentReleaseComponentRole,
} from '@orchbee/ssh-environment-protocol';

import {
	verifySshEnvironmentArtifactAgainstCatalog,
	verifySshEnvironmentReleaseCatalog,
	type VerifiedSshEnvironmentArtifact,
	type VerifiedSshEnvironmentReleaseCatalog,
} from './verify-ssh-environment-release';

const MAXIMUM_TRUST_DESCRIPTOR_FILE_BYTES = 16 * 1024;
const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

export type SshEnvironmentReleaseCatalogLoadResult =
	| { readonly catalog: VerifiedSshEnvironmentReleaseCatalog; readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export type SshEnvironmentArtifactLoadResult =
	| { readonly artifact: VerifiedSshEnvironmentArtifact; readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export interface SshEnvironmentReleaseSource {
	loadArtifact(
		role: SshEnvironmentReleaseComponentRole,
		platform: SshEnvironmentPlatform,
	): Promise<SshEnvironmentArtifactLoadResult>;
	loadCatalog(): Promise<SshEnvironmentReleaseCatalogLoadResult>;
}

export interface PackagedSshEnvironmentReleaseSourceOptions {
	readonly releaseDirectoryPath: string;
	readonly trustDescriptorFilePath: string;
}

export class PackagedSshEnvironmentReleaseSource implements SshEnvironmentReleaseSource {
	readonly #artifactLoads = new Map<string, Promise<SshEnvironmentArtifactLoadResult>>();
	#catalogLoad: Promise<SshEnvironmentReleaseCatalogLoadResult> | undefined;
	readonly #releaseDirectoryPath: string;
	readonly #trustDescriptorFilePath: string;

	constructor(options: PackagedSshEnvironmentReleaseSourceOptions) {
		if (!isAbsolute(options.releaseDirectoryPath) || !isAbsolute(options.trustDescriptorFilePath)) {
			throw new Error('SSH Environment release resource paths must be absolute.');
		}
		this.#releaseDirectoryPath = options.releaseDirectoryPath;
		this.#trustDescriptorFilePath = options.trustDescriptorFilePath;
	}

	loadArtifact(
		role: SshEnvironmentReleaseComponentRole,
		platform: SshEnvironmentPlatform,
	): Promise<SshEnvironmentArtifactLoadResult> {
		const key = `${role}:${platform}`;
		const existing = this.#artifactLoads.get(key);
		if (existing !== undefined) return existing;
		const load = this.#loadArtifact(role, platform);
		this.#artifactLoads.set(key, load);
		return load;
	}

	loadCatalog(): Promise<SshEnvironmentReleaseCatalogLoadResult> {
		this.#catalogLoad ??= this.#loadCatalog();
		return this.#catalogLoad;
	}

	async #loadArtifact(
		role: SshEnvironmentReleaseComponentRole,
		platform: SshEnvironmentPlatform,
	): Promise<SshEnvironmentArtifactLoadResult> {
		try {
			const catalog = await this.loadCatalog();
			if (catalog.kind === 'unavailable') return catalog;
			const artifactBytes = await readRegularFile(
				join(
					this.#releaseDirectoryPath,
					SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS[role][platform],
				),
				SSH_ENVIRONMENT_RELEASE_MAXIMUM_ARTIFACT_BYTES,
			);
			const verification = verifySshEnvironmentArtifactAgainstCatalog({
				artifactBytes,
				catalog: catalog.catalog,
				expectedPlatform: platform,
				expectedRole: role,
			});
			return verification.kind === 'verified'
				? { artifact: verification.artifact, kind: 'ready' }
				: { kind: 'unavailable' };
		} catch {
			return { kind: 'unavailable' };
		}
	}

	async #loadCatalog(): Promise<SshEnvironmentReleaseCatalogLoadResult> {
		try {
			await assertReleaseLayout(this.#releaseDirectoryPath);
			const [manifestBytes, trustDescriptorBytes] = await Promise.all([
				readRegularFile(
					join(this.#releaseDirectoryPath, SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME),
					SSH_ENVIRONMENT_RELEASE_MAXIMUM_MANIFEST_BYTES,
				),
				readRegularFile(this.#trustDescriptorFilePath, MAXIMUM_TRUST_DESCRIPTOR_FILE_BYTES),
			]);
			const trustDescriptor = decodeCanonicalTrustDescriptor(trustDescriptorBytes);
			if (trustDescriptor === null) return { kind: 'unavailable' };
			const verification = verifySshEnvironmentReleaseCatalog({
				manifestBytes,
				trustDescriptor,
			});
			if (verification.kind === 'failed') return { kind: 'unavailable' };
			for (const role of SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES) {
				for (const platform of SSH_ENVIRONMENT_RELEASE_PLATFORMS) {
					const metadata = await lstat(
						join(
							this.#releaseDirectoryPath,
							SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS[role][platform],
						),
					);
					if (
						!metadata.isFile() ||
						metadata.size !== verification.catalog.components[role].artifacts[platform].sizeBytes
					) {
						return { kind: 'unavailable' };
					}
				}
			}
			return {
				catalog: verification.catalog,
				kind: 'ready',
			};
		} catch {
			return { kind: 'unavailable' };
		}
	}
}

export class UnavailableSshEnvironmentReleaseSource implements SshEnvironmentReleaseSource {
	async loadArtifact(
		_role: SshEnvironmentReleaseComponentRole,
		_platform: SshEnvironmentPlatform,
	): Promise<SshEnvironmentArtifactLoadResult> {
		return { kind: 'unavailable' };
	}

	async loadCatalog(): Promise<SshEnvironmentReleaseCatalogLoadResult> {
		return { kind: 'unavailable' };
	}
}

async function assertReleaseLayout(root: string): Promise<void> {
	await assertExactDirectory(root, ['artifacts', SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME]);
	const artifactsRoot = join(root, 'artifacts');
	await assertExactDirectory(artifactsRoot, [...SSH_ENVIRONMENT_RELEASE_PLATFORMS]);
	const artifactNames = SSH_ENVIRONMENT_RELEASE_COMPONENT_ROLES.map(
		(role) => SSH_ENVIRONMENT_RELEASE_ARTIFACT_FILE_NAMES[role],
	);
	for (const platform of SSH_ENVIRONMENT_RELEASE_PLATFORMS) {
		await assertExactDirectory(join(artifactsRoot, platform), artifactNames);
	}
}

async function assertExactDirectory(directory: string, expectedEntries: string[]): Promise<void> {
	const metadata = await lstat(directory);
	if (!metadata.isDirectory()) throw new Error('SSH Environment release directory is invalid.');
	const actual = (await readdir(directory)).sort();
	const expected = expectedEntries.sort();
	if (JSON.stringify(actual) !== JSON.stringify(expected)) {
		throw new Error('SSH Environment release layout is invalid.');
	}
}

async function readRegularFile(filePath: string, maximumBytes: number): Promise<Buffer> {
	const metadata = await lstat(filePath);
	if (!metadata.isFile() || metadata.size > maximumBytes) {
		throw new Error('SSH Environment release resource is invalid.');
	}
	const bytes = await readFile(filePath);
	if (bytes.byteLength === 0 || bytes.byteLength > maximumBytes) {
		throw new Error('SSH Environment release resource is invalid.');
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
	const parsed = SshEnvironmentReleaseTrustDescriptorSchema.safeParse(value);
	if (!parsed.success) return null;
	return Buffer.from(encodeSshEnvironmentReleaseTrustDescriptor(parsed.data)).equals(
		Buffer.from(bytes),
	)
		? parsed.data
		: null;
}
