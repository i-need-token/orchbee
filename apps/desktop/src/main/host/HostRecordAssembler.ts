import {
	PROTOCOL_LIMITS,
	TRANSPORT_ENVELOPE_HEADER_BYTES,
	TRANSPORT_ENVELOPE_MAGIC,
} from '@orchbee/terminal-protocol';

const TRANSPORT_TOTAL_LENGTH_OFFSET = 8;
const TRANSPORT_LENGTH_FIELD_END = TRANSPORT_TOTAL_LENGTH_OFFSET + Uint32Array.BYTES_PER_ELEMENT;

export class HostRecordAssembler {
	readonly #chunks: Uint8Array[] = [];
	readonly #onRecord: (record: Uint8Array) => void;
	#bufferedBytes = 0;
	#headOffset = 0;

	constructor(onRecord: (record: Uint8Array) => void) {
		this.#onRecord = onRecord;
	}

	get bufferedBytes(): number {
		return this.#bufferedBytes;
	}

	push(chunk: Uint8Array): void {
		if (chunk.byteLength === 0) return;
		this.#chunks.push(chunk);
		this.#bufferedBytes += chunk.byteLength;

		while (this.#bufferedBytes >= TRANSPORT_LENGTH_FIELD_END) {
			const header = this.#peek(TRANSPORT_LENGTH_FIELD_END);
			for (const [index, expected] of TRANSPORT_ENVELOPE_MAGIC.entries()) {
				if (header[index] !== expected) throw new Error('Host transport record has invalid magic.');
			}
			const totalLength = new DataView(
				header.buffer,
				header.byteOffset,
				header.byteLength,
			).getUint32(TRANSPORT_TOTAL_LENGTH_OFFSET, true);
			if (
				totalLength < TRANSPORT_ENVELOPE_HEADER_BYTES ||
				totalLength > PROTOCOL_LIMITS.maxTransportRecordBytes
			) {
				throw new Error(`Host transport record length ${totalLength} is outside its bound.`);
			}
			if (this.#bufferedBytes < totalLength) return;
			this.#onRecord(this.#take(totalLength));
		}
	}

	finish(): void {
		if (this.#bufferedBytes !== 0) {
			throw new Error('Host transport closed with a truncated record.');
		}
	}

	#peek(length: number): Uint8Array {
		const first = this.#chunks[0];
		if (first !== undefined && first.byteLength - this.#headOffset >= length) {
			return first.subarray(this.#headOffset, this.#headOffset + length);
		}
		const bytes = new Uint8Array(length);
		let copied = 0;
		let chunkIndex = 0;
		let chunkOffset = this.#headOffset;
		while (copied < length) {
			const chunk = this.#chunks[chunkIndex];
			if (chunk === undefined) throw new Error('Host record assembler invariant failed.');
			const available = Math.min(length - copied, chunk.byteLength - chunkOffset);
			bytes.set(chunk.subarray(chunkOffset, chunkOffset + available), copied);
			copied += available;
			chunkIndex += 1;
			chunkOffset = 0;
		}
		return bytes;
	}

	#take(length: number): Uint8Array {
		const first = this.#chunks[0];
		if (first === undefined) throw new Error('Host record assembler invariant failed.');
		const firstRemaining = first.byteLength - this.#headOffset;
		if (firstRemaining >= length) {
			const record = first.subarray(this.#headOffset, this.#headOffset + length);
			this.#consume(length);
			return record;
		}

		const record = new Uint8Array(length);
		let copied = 0;
		while (copied < length) {
			const chunk = this.#chunks[0];
			if (chunk === undefined) throw new Error('Host record assembler invariant failed.');
			const available = Math.min(length - copied, chunk.byteLength - this.#headOffset);
			record.set(chunk.subarray(this.#headOffset, this.#headOffset + available), copied);
			this.#consume(available);
			copied += available;
		}
		return record;
	}

	#consume(length: number): void {
		this.#bufferedBytes -= length;
		this.#headOffset += length;
		const first = this.#chunks[0];
		if (first !== undefined && this.#headOffset === first.byteLength) {
			this.#chunks.shift();
			this.#headOffset = 0;
		}
	}
}
