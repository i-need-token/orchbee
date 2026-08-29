import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME,
	SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX,
	SshEnvironmentBuildDigestSchema,
	SshRuntimeReleaseManifestSchema,
	SshRuntimeReleaseTrustDescriptorSchema,
	SshRuntimeReleaseUnsignedManifestSchema,
	createSshRuntimeManifestSigningPayload,
	encodeSshRuntimeReleaseManifest,
	encodeSshRuntimeReleaseTrustDescriptor,
} from '@orchbee/ssh-environment-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import {
	PackagedSshRuntimeReleaseSource,
	UnavailableSshRuntimeReleaseSource,
} from '../ssh-runtime-release-source';

const temporaryRoots = new Set<string>();
const darwinBytes = Buffer.from('darwin runtime');
const linuxBytes = Buffer.from('linux runtime');

afterEach(async () => {
	await Promise.all([...temporaryRoots].map((root) => rm(root, { force: true, recursive: true })));
	temporaryRoots.clear();
});

describe('packaged SSH Runtime release source', () => {
	it('verifies the pinned catalog before lazily loading and caching selected artifact bytes', async () => {
		const fixture = await createFixture();
		const source = new PackagedSshRuntimeReleaseSource(fixture);

		const catalog = await source.loadCatalog();
		expect(catalog).toMatchObject({
			kind: 'ready',
			catalog: {
				artifacts: {
					'darwin-arm64': { sizeBytes: darwinBytes.byteLength },
					'linux-x64': { sizeBytes: linuxBytes.byteLength },
				},
				runtimeVersion: '0.1.0',
			},
		});

		const first = await source.loadArtifact('darwin-arm64');
		expect(first).toMatchObject({ artifact: { platform: 'darwin-arm64' }, kind: 'ready' });
		if (first.kind !== 'ready') throw new Error('Expected a verified Runtime artifact.');
		expect(Buffer.from(first.artifact.bytes)).toEqual(darwinBytes);
		await writeFile(
			path.join(
				fixture.releaseDirectoryPath,
				SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64'],
			),
			Buffer.alloc(darwinBytes.byteLength, 0),
		);
		await expect(source.loadArtifact('darwin-arm64')).resolves.toBe(first);
	});

	it('fails closed on a mutated artifact, noncanonical trust, and unexpected layout entry', async () => {
		const mutated = await createFixture();
		await writeFile(
			path.join(
				mutated.releaseDirectoryPath,
				SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['linux-x64'],
			),
			Buffer.alloc(linuxBytes.byteLength, 0),
		);
		const mutatedSource = new PackagedSshRuntimeReleaseSource(mutated);
		await expect(mutatedSource.loadCatalog()).resolves.toMatchObject({ kind: 'ready' });
		await expect(mutatedSource.loadArtifact('linux-x64')).resolves.toEqual({
			kind: 'unavailable',
		});

		const noncanonical = await createFixture();
		const trustValue = JSON.parse(
			await readFile(noncanonical.trustDescriptorFilePath, 'utf8'),
		) as unknown;
		await writeFile(noncanonical.trustDescriptorFilePath, JSON.stringify(trustValue, null, 2));
		await expect(new PackagedSshRuntimeReleaseSource(noncanonical).loadCatalog()).resolves.toEqual({
			kind: 'unavailable',
		});

		const unexpected = await createFixture();
		await writeFile(path.join(unexpected.releaseDirectoryPath, 'unexpected'), 'bad');
		await expect(new PackagedSshRuntimeReleaseSource(unexpected).loadCatalog()).resolves.toEqual({
			kind: 'unavailable',
		});
	});

	it('keeps an explicitly unavailable development source side-effect free', async () => {
		const source = new UnavailableSshRuntimeReleaseSource();
		await expect(source.loadCatalog()).resolves.toEqual({ kind: 'unavailable' });
		await expect(source.loadArtifact('darwin-arm64')).resolves.toEqual({ kind: 'unavailable' });
	});

	it('requires absolute package resource paths', () => {
		expect(
			() =>
				new PackagedSshRuntimeReleaseSource({
					releaseDirectoryPath: 'relative-release',
					trustDescriptorFilePath: '/absolute/trust.json',
				}),
		).toThrow('absolute');
	});
});

async function createFixture() {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-runtime-source-'));
	temporaryRoots.add(root);
	const releaseDirectoryPath = path.join(root, 'release');
	const trustDescriptorFilePath = path.join(root, 'app.asar', 'dist', 'main', 'trust.json');
	const signer = generateKeyPairSync('ed25519');
	const unsigned = SshRuntimeReleaseUnsignedManifestSchema.parse({
		artifacts: [
			{
				platform: 'darwin-arm64',
				sha256: digest(darwinBytes),
				sizeBytes: darwinBytes.byteLength,
			},
			{
				platform: 'linux-x64',
				sha256: digest(linuxBytes),
				sizeBytes: linuxBytes.byteLength,
			},
		],
		runtimeVersion: '0.1.0',
		schemaVersion: 1,
	});
	const manifest = SshRuntimeReleaseManifestSchema.parse({
		...unsigned,
		signature: sign(
			null,
			createSshRuntimeManifestSigningPayload(unsigned),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = encodeSshRuntimeReleaseManifest(manifest);
	const publicKeySpki = signer.publicKey.export({ format: 'der', type: 'spki' });
	const trustDescriptor = SshRuntimeReleaseTrustDescriptorSchema.parse({
		manifestSha256: digest(manifestBytes),
		publicKeySpki: publicKeySpki.toString('base64url'),
		schemaVersion: 1,
		signingKeyId: `${SSH_RUNTIME_RELEASE_SIGNING_KEY_ID_PREFIX}${digest(publicKeySpki)}`,
	});
	await Promise.all([
		mkdir(
			path.dirname(
				path.join(
					releaseDirectoryPath,
					SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64'],
				),
			),
			{ recursive: true },
		),
		mkdir(
			path.dirname(
				path.join(releaseDirectoryPath, SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['linux-x64']),
			),
			{ recursive: true },
		),
		mkdir(path.dirname(trustDescriptorFilePath), { recursive: true }),
	]);
	await Promise.all([
		writeFile(
			path.join(releaseDirectoryPath, SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['darwin-arm64']),
			darwinBytes,
		),
		writeFile(
			path.join(releaseDirectoryPath, SSH_RUNTIME_RELEASE_ARTIFACT_RELATIVE_PATHS['linux-x64']),
			linuxBytes,
		),
		writeFile(
			path.join(releaseDirectoryPath, SSH_RUNTIME_RELEASE_MANIFEST_FILE_NAME),
			manifestBytes,
		),
		writeFile(trustDescriptorFilePath, encodeSshRuntimeReleaseTrustDescriptor(trustDescriptor)),
	]);
	return { releaseDirectoryPath, trustDescriptorFilePath };
}

function digest(bytes: Uint8Array) {
	return SshEnvironmentBuildDigestSchema.parse(createHash('sha256').update(bytes).digest('hex'));
}
