import type { SshEnvironmentPlatform } from '@orchbee/ssh-environment-protocol';

const SSH_RUNTIME_BOOTSTRAP_MAXIMUM_STATUS_BYTES = 128;

export type SshRuntimeBootstrapArtifactDisposition = 'present' | 'upload';

export interface SshRuntimeBootstrapStatus {
	readonly artifactDisposition: SshRuntimeBootstrapArtifactDisposition;
	readonly platform: SshEnvironmentPlatform;
}

export const SSH_RUNTIME_BOOTSTRAP_STATUS_LINES = Object.freeze({
	'darwin-arm64': Object.freeze({
		present: 'OBSI/1 darwin-arm64 present',
		upload: 'OBSI/1 darwin-arm64 upload',
	}),
	'linux-x64': Object.freeze({
		present: 'OBSI/1 linux-x64 present',
		upload: 'OBSI/1 linux-x64 upload',
	}),
} satisfies Readonly<
	Record<SshEnvironmentPlatform, Readonly<Record<SshRuntimeBootstrapArtifactDisposition, string>>>
>);

type SshRuntimeBootstrapStatusDecodeResult =
	| { readonly kind: 'pending' }
	| { readonly kind: 'rejected' }
	| { readonly kind: 'status'; readonly status: SshRuntimeBootstrapStatus };

const statuses = new Map<string, SshRuntimeBootstrapStatus>([
	[
		SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present,
		{ artifactDisposition: 'present', platform: 'darwin-arm64' },
	],
	[
		SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].upload,
		{ artifactDisposition: 'upload', platform: 'darwin-arm64' },
	],
	[
		SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['linux-x64'].present,
		{ artifactDisposition: 'present', platform: 'linux-x64' },
	],
	[
		SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['linux-x64'].upload,
		{ artifactDisposition: 'upload', platform: 'linux-x64' },
	],
]);

export class SshRuntimeBootstrapStatusDecoder {
	#bytes = Buffer.alloc(0);
	#finished = false;

	push(chunk: Uint8Array): SshRuntimeBootstrapStatusDecodeResult {
		if (this.#finished || chunk.byteLength === 0) return { kind: 'rejected' };
		this.#bytes = Buffer.concat([this.#bytes, chunk]);
		if (this.#bytes.byteLength > SSH_RUNTIME_BOOTSTRAP_MAXIMUM_STATUS_BYTES) {
			this.#finished = true;
			return { kind: 'rejected' };
		}
		const newline = this.#bytes.indexOf(0x0a);
		if (newline < 0) return { kind: 'pending' };
		this.#finished = true;
		if (newline !== this.#bytes.byteLength - 1) return { kind: 'rejected' };
		const line = this.#bytes.subarray(0, newline).toString('ascii');
		if (!Buffer.from(line, 'ascii').equals(this.#bytes.subarray(0, newline))) {
			return { kind: 'rejected' };
		}
		const status = statuses.get(line);
		return status === undefined ? { kind: 'rejected' } : { kind: 'status', status };
	}
}
