import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS,
	SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME,
	SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX,
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentReleaseManifestSchema,
	SshEnvironmentReleaseTrustDescriptorSchema,
	SshEnvironmentReleaseUnsignedManifestSchema,
	createSshEnvironmentReleaseManifestSigningPayload,
	encodeSshEnvironmentReleaseManifest,
	encodeSshEnvironmentReleaseTrustDescriptor,
} from '@orchbee/ssh-environment-protocol';
import { afterEach, describe, expect, it } from 'vitest';

import {
	PackagedSshEnvironmentReleaseSource,
	UnavailableSshEnvironmentReleaseSource,
} from '../ssh-environment-release-source';

const temporaryRoots = new Set<string>();
const artifactBytes = {
	'ssh-environment-runtime': {
		'darwin-arm64': Buffer.from('darwin runtime'),
		'linux-x64': Buffer.from('linux runtime'),
	},
	'terminal-host': {
		'darwin-arm64': Buffer.from('darwin terminal host'),
		'linux-x64': Buffer.from('linux terminal host'),
	},
};

afterEach(async () => {
	await Promise.all([...temporaryRoots].map((root) => rm(root, { force: true, recursive: true })));
	temporaryRoots.clear();
});

describe('packaged SSH Environment release source', () => {
	it('verifies the complete v2 cohort before lazily loading and caching each component', async () => {
		const fixture = await createFixture();
		const source = new PackagedSshEnvironmentReleaseSource(fixture);

		const catalog = await source.loadCatalog();
		expect(catalog).toMatchObject({
			kind: 'ready',
			catalog: {
				components: {
					'ssh-environment-runtime': {
						artifacts: {
							'darwin-arm64': {
								sizeBytes: artifactBytes['ssh-environment-runtime']['darwin-arm64'].byteLength,
							},
							'linux-x64': {
								sizeBytes: artifactBytes['ssh-environment-runtime']['linux-x64'].byteLength,
							},
						},
						version: '1.2.3',
					},
					'terminal-host': {
						version: '4.5.6',
					},
				},
			},
		});

		const first = await source.loadArtifact('ssh-environment-runtime', 'darwin-arm64');
		expect(first).toMatchObject({ artifact: { platform: 'darwin-arm64' }, kind: 'ready' });
		if (first.kind !== 'ready') throw new Error('Expected a verified Runtime artifact.');
		expect(Buffer.from(first.artifact.bytes)).toEqual(
			artifactBytes['ssh-environment-runtime']['darwin-arm64'],
		);
		await writeFile(
			path.join(
				fixture.releaseDirectoryPath,
				SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['ssh-environment-runtime']['darwin-arm64'],
			),
			Buffer.alloc(artifactBytes['ssh-environment-runtime']['darwin-arm64'].byteLength, 0),
		);
		await expect(source.loadArtifact('ssh-environment-runtime', 'darwin-arm64')).resolves.toBe(
			first,
		);
	});

	it('fails closed on a changed Runtime, missing Terminal Host, noncanonical trust, or layout drift', async () => {
		const mutated = await createFixture();
		await writeFile(
			path.join(
				mutated.releaseDirectoryPath,
				SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['ssh-environment-runtime']['linux-x64'],
			),
			Buffer.alloc(artifactBytes['ssh-environment-runtime']['linux-x64'].byteLength, 0),
		);
		const mutatedSource = new PackagedSshEnvironmentReleaseSource(mutated);
		await expect(mutatedSource.loadCatalog()).resolves.toMatchObject({ kind: 'ready' });
		await expect(
			mutatedSource.loadArtifact('ssh-environment-runtime', 'linux-x64'),
		).resolves.toEqual({
			kind: 'unavailable',
		});

		const missingCompanion = await createFixture();
		await rm(
			path.join(
				missingCompanion.releaseDirectoryPath,
				SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS['terminal-host']['darwin-arm64'],
			),
		);
		await expect(
			new PackagedSshEnvironmentReleaseSource(missingCompanion).loadCatalog(),
		).resolves.toEqual({ kind: 'unavailable' });

		const noncanonical = await createFixture();
		const trustValue: unknown = JSON.parse(
			await readFile(noncanonical.trustDescriptorFilePath, 'utf8'),
		);
		await writeFile(noncanonical.trustDescriptorFilePath, JSON.stringify(trustValue, null, 2));
		await expect(
			new PackagedSshEnvironmentReleaseSource(noncanonical).loadCatalog(),
		).resolves.toEqual({ kind: 'unavailable' });

		const unexpected = await createFixture();
		await writeFile(path.join(unexpected.releaseDirectoryPath, 'unexpected'), 'bad');
		await expect(
			new PackagedSshEnvironmentReleaseSource(unexpected).loadCatalog(),
		).resolves.toEqual({
			kind: 'unavailable',
		});
	});

	it('keeps an explicitly unavailable development source side-effect free', async () => {
		const source = new UnavailableSshEnvironmentReleaseSource();
		await expect(source.loadCatalog()).resolves.toEqual({ kind: 'unavailable' });
		await expect(source.loadArtifact('terminal-host', 'darwin-arm64')).resolves.toEqual({
			kind: 'unavailable',
		});
	});

	it('requires absolute package resource paths', () => {
		expect(
			() =>
				new PackagedSshEnvironmentReleaseSource({
					releaseDirectoryPath: 'relative-release',
					trustDescriptorFilePath: '/absolute/trust.json',
				}),
		).toThrow('absolute');
	});
});

async function createFixture() {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-environment-release-source-'));
	temporaryRoots.add(root);
	const releaseDirectoryPath = path.join(root, 'release');
	const trustDescriptorFilePath = path.join(root, 'app.asar', 'dist', 'main', 'trust.json');
	const signer = generateKeyPairSync('ed25519');
	const unsigned = SshEnvironmentReleaseUnsignedManifestSchema.parse({
		schemaVersion: 2,
		components: [
			{
				role: 'ssh-environment-runtime',
				version: '1.2.3',
				artifacts: createArtifactRecords(artifactBytes['ssh-environment-runtime']),
			},
			{
				role: 'terminal-host',
				version: '4.5.6',
				artifacts: createArtifactRecords(artifactBytes['terminal-host']),
			},
		],
	});
	const manifest = SshEnvironmentReleaseManifestSchema.parse({
		...unsigned,
		signature: sign(
			null,
			createSshEnvironmentReleaseManifestSigningPayload(unsigned),
			signer.privateKey,
		).toString('base64url'),
	});
	const manifestBytes = encodeSshEnvironmentReleaseManifest(manifest);
	const publicKeySpki = signer.publicKey.export({ format: 'der', type: 'spki' });
	const trustDescriptor = SshEnvironmentReleaseTrustDescriptorSchema.parse({
		manifestSha256: digest(manifestBytes),
		publicKeySpki: publicKeySpki.toString('base64url'),
		schemaVersion: 2,
		signingKeyId: `${SSH_ENVIRONMENT_RELEASE_SIGNING_KEY_ID_PREFIX}${digest(publicKeySpki)}`,
	});
	for (const [role, components] of Object.entries(artifactBytes)) {
		for (const [platform, bytes] of Object.entries(components)) {
			const artifactPath = path.join(
				releaseDirectoryPath,
				SSH_ENVIRONMENT_RELEASE_ARTIFACT_RELATIVE_PATHS[
					role === 'terminal-host' ? 'terminal-host' : 'ssh-environment-runtime'
				][platform === 'linux-x64' ? 'linux-x64' : 'darwin-arm64'],
			);
			await mkdir(path.dirname(artifactPath), { recursive: true });
			await writeFile(artifactPath, bytes);
		}
	}
	await mkdir(path.dirname(trustDescriptorFilePath), { recursive: true });
	await Promise.all([
		writeFile(
			path.join(releaseDirectoryPath, SSH_ENVIRONMENT_RELEASE_MANIFEST_FILE_NAME),
			manifestBytes,
		),
		writeFile(trustDescriptorFilePath, encodeSshEnvironmentReleaseTrustDescriptor(trustDescriptor)),
	]);
	return { releaseDirectoryPath, trustDescriptorFilePath };
}

function createArtifactRecords(componentBytes: {
	readonly 'darwin-arm64': Uint8Array;
	readonly 'linux-x64': Uint8Array;
}) {
	return [
		{
			platform: 'darwin-arm64',
			sha256: digest(componentBytes['darwin-arm64']),
			sizeBytes: componentBytes['darwin-arm64'].byteLength,
		},
		{
			platform: 'linux-x64',
			sha256: digest(componentBytes['linux-x64']),
			sizeBytes: componentBytes['linux-x64'].byteLength,
		},
	];
}

function digest(bytes: Uint8Array) {
	return SshEnvironmentBuildDigestSchema.parse(createHash('sha256').update(bytes).digest('hex'));
}
