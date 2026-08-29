import {
	SSH_ENVIRONMENT_FRAME_HEADER_BYTES,
	SSH_ENVIRONMENT_MAX_FRAME_BYTES,
	decodeSshEnvironmentFrame,
	type SshEnvironmentDecodeError,
	type SshEnvironmentMessage,
} from './ssh-environment-protocol';

export type SshEnvironmentFrameDecoderResult =
	| { readonly ok: true; readonly messages: readonly SshEnvironmentMessage[] }
	| { readonly ok: false; readonly error: SshEnvironmentDecodeError };

export class SshEnvironmentFrameDecoder {
	private readonly pending = new Uint8Array(SSH_ENVIRONMENT_MAX_FRAME_BYTES);
	private pendingLength = 0;
	private terminalError: SshEnvironmentDecodeError | undefined;

	public push(chunk: Uint8Array): SshEnvironmentFrameDecoderResult {
		if (this.terminalError !== undefined) {
			return { ok: false, error: this.terminalError };
		}
		if (chunk.byteLength === 0) {
			return { ok: true, messages: [] };
		}

		const messages: SshEnvironmentMessage[] = [];
		let offset = 0;
		while (offset < chunk.byteLength) {
			if (this.pendingLength < SSH_ENVIRONMENT_FRAME_HEADER_BYTES) {
				const requiredHeaderBytes = SSH_ENVIRONMENT_FRAME_HEADER_BYTES - this.pendingLength;
				const acceptedBytes = Math.min(requiredHeaderBytes, chunk.byteLength - offset);
				this.pending.set(chunk.subarray(offset, offset + acceptedBytes), this.pendingLength);
				this.pendingLength += acceptedBytes;
				offset += acceptedBytes;
				if (this.pendingLength < SSH_ENVIRONMENT_FRAME_HEADER_BYTES) break;
			}

			const payloadLength = new DataView(this.pending.buffer).getUint32(8, false);
			const frameLength = SSH_ENVIRONMENT_FRAME_HEADER_BYTES + payloadLength;
			if (frameLength > SSH_ENVIRONMENT_MAX_FRAME_BYTES) {
				return this.fail({
					code: 'frame-too-large',
					detail: 'declared payload exceeds the maximum size',
				});
			}

			const remainingFrameBytes = frameLength - this.pendingLength;
			if (remainingFrameBytes > 0) {
				const acceptedBytes = Math.min(remainingFrameBytes, chunk.byteLength - offset);
				this.pending.set(chunk.subarray(offset, offset + acceptedBytes), this.pendingLength);
				this.pendingLength += acceptedBytes;
				offset += acceptedBytes;
			}
			if (this.pendingLength < frameLength) {
				break;
			}

			const decoded = decodeSshEnvironmentFrame(this.pending.slice(0, frameLength));
			if (!decoded.ok) {
				return this.fail(decoded.error);
			}
			messages.push(decoded.message);
			this.pendingLength = 0;
		}
		return { ok: true, messages };
	}

	public finish(): SshEnvironmentFrameDecoderResult {
		if (this.terminalError !== undefined) {
			return { ok: false, error: this.terminalError };
		}
		if (this.pendingLength !== 0) {
			return this.fail({
				code: 'frame-length-mismatch',
				detail: 'transport ended with a partial frame',
			});
		}
		return { ok: true, messages: [] };
	}

	private fail(error: SshEnvironmentDecodeError): SshEnvironmentFrameDecoderResult {
		this.terminalError = error;
		this.pendingLength = 0;
		return { ok: false, error };
	}
}
