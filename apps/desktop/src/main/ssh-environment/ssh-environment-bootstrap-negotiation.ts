import type {
	SshEnvironmentPlatform,
	SshEnvironmentReleaseComponentRole,
} from '@orchbee/ssh-environment-protocol';

const SSH_ENVIRONMENT_BOOTSTRAP_MAXIMUM_LINE_BYTES = 128;

export type SshEnvironmentBootstrapArtifactDisposition = 'present' | 'upload';

export interface SshEnvironmentBootstrapArtifactStatus {
	readonly artifactDisposition: SshEnvironmentBootstrapArtifactDisposition;
	readonly platform: SshEnvironmentPlatform;
	readonly role: SshEnvironmentReleaseComponentRole;
	readonly type: 'artifact-status';
}

interface SshEnvironmentBootstrapCommitStatus {
	readonly platform: SshEnvironmentPlatform;
	readonly type: 'cohort-committed';
}

export type SshEnvironmentBootstrapMessage =
	| SshEnvironmentBootstrapArtifactStatus
	| SshEnvironmentBootstrapCommitStatus;

export const SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES = Object.freeze({
	'darwin-arm64': Object.freeze({
		'ssh-environment-runtime': Object.freeze({
			present: 'OBSI/2 darwin-arm64 ssh-environment-runtime present',
			upload: 'OBSI/2 darwin-arm64 ssh-environment-runtime upload',
		}),
		'terminal-host': Object.freeze({
			present: 'OBSI/2 darwin-arm64 terminal-host present',
			upload: 'OBSI/2 darwin-arm64 terminal-host upload',
		}),
		committed: 'OBSC/2 darwin-arm64 ready',
	}),
	'linux-x64': Object.freeze({
		'ssh-environment-runtime': Object.freeze({
			present: 'OBSI/2 linux-x64 ssh-environment-runtime present',
			upload: 'OBSI/2 linux-x64 ssh-environment-runtime upload',
		}),
		'terminal-host': Object.freeze({
			present: 'OBSI/2 linux-x64 terminal-host present',
			upload: 'OBSI/2 linux-x64 terminal-host upload',
		}),
		committed: 'OBSC/2 linux-x64 ready',
	}),
} satisfies Readonly<
	Record<
		SshEnvironmentPlatform,
		Readonly<
			Record<
				SshEnvironmentReleaseComponentRole,
				Readonly<Record<SshEnvironmentBootstrapArtifactDisposition, string>>
			> & { readonly committed: string }
		>
	>
>);

export type SshEnvironmentBootstrapDecodeResult =
	| { readonly kind: 'messages'; readonly messages: readonly SshEnvironmentBootstrapMessage[] }
	| { readonly kind: 'rejected' };

const messagesByLine = createMessagesByLine();

export class SshEnvironmentBootstrapDecoder {
	#bytes = Buffer.alloc(0);
	#finished = false;

	push(chunk: Uint8Array): SshEnvironmentBootstrapDecodeResult {
		if (this.#finished || chunk.byteLength === 0) return { kind: 'rejected' };
		this.#bytes = Buffer.concat([this.#bytes, chunk]);
		const messages: SshEnvironmentBootstrapMessage[] = [];
		while (true) {
			const newline = this.#bytes.indexOf(0x0a);
			if (newline < 0) {
				if (this.#bytes.byteLength >= SSH_ENVIRONMENT_BOOTSTRAP_MAXIMUM_LINE_BYTES) {
					this.#finished = true;
					return { kind: 'rejected' };
				}
				return { kind: 'messages', messages };
			}
			if (newline + 1 > SSH_ENVIRONMENT_BOOTSTRAP_MAXIMUM_LINE_BYTES) {
				this.#finished = true;
				return { kind: 'rejected' };
			}
			const lineBytes = this.#bytes.subarray(0, newline);
			const line = lineBytes.toString('ascii');
			if (!Buffer.from(line, 'ascii').equals(lineBytes)) {
				this.#finished = true;
				return { kind: 'rejected' };
			}
			const message = messagesByLine.get(line);
			if (message === undefined) {
				this.#finished = true;
				return { kind: 'rejected' };
			}
			messages.push(message);
			this.#bytes = this.#bytes.subarray(newline + 1);
			if (message.type === 'cohort-committed') {
				this.#finished = true;
				if (this.#bytes.byteLength !== 0) return { kind: 'rejected' };
				return { kind: 'messages', messages };
			}
		}
	}
}

function createMessagesByLine(): ReadonlyMap<string, SshEnvironmentBootstrapMessage> {
	const messages = new Map<string, SshEnvironmentBootstrapMessage>();
	for (const platform of ['darwin-arm64', 'linux-x64'] as const) {
		for (const role of ['ssh-environment-runtime', 'terminal-host'] as const) {
			for (const artifactDisposition of ['present', 'upload'] as const) {
				messages.set(SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[platform][role][artifactDisposition], {
					artifactDisposition,
					platform,
					role,
					type: 'artifact-status',
				});
			}
		}
		messages.set(SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[platform].committed, {
			platform,
			type: 'cohort-committed',
		});
	}
	return messages;
}
