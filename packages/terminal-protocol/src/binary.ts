import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolFailure, ProtocolResult } from './protocol-result';

const emptyBytes = new Uint8Array();
const sharedArrayBufferByteLengthGetter =
	typeof SharedArrayBuffer === 'function'
		? Object.getOwnPropertyDescriptor(SharedArrayBuffer.prototype, 'byteLength')?.get
		: undefined;

export class BinaryReader {
	readonly #bytes: Uint8Array;
	readonly #view: DataView;
	#failure: ProtocolFailure | null = null;
	#offset: number;

	constructor(bytes: Uint8Array, offset = 0) {
		this.#bytes = bytes;
		this.#view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
		this.#offset = offset;
	}

	get failure(): ProtocolFailure | null {
		return this.#failure;
	}

	get offset(): number {
		return this.#offset;
	}

	readBytes(length: number): Uint8Array {
		if (!this.#ensure(length)) return emptyBytes;
		const start = this.#offset;
		this.#offset += length;
		return this.#bytes.subarray(start, this.#offset);
	}

	readUint8(): number {
		if (!this.#ensure(1)) return 0;
		const value = this.#view.getUint8(this.#offset);
		this.#offset += 1;
		return value;
	}

	readUint16(): number {
		if (!this.#ensure(2)) return 0;
		const value = this.#view.getUint16(this.#offset, true);
		this.#offset += 2;
		return value;
	}

	readUint32(): number {
		if (!this.#ensure(4)) return 0;
		const value = this.#view.getUint32(this.#offset, true);
		this.#offset += 4;
		return value;
	}

	readUint64(): bigint {
		if (!this.#ensure(8)) return 0n;
		const value = this.#view.getBigUint64(this.#offset, true);
		this.#offset += 8;
		return value;
	}

	readFloat32(): number {
		if (!this.#ensure(4)) return 0;
		const value = this.#view.getFloat32(this.#offset, true);
		this.#offset += 4;
		return value;
	}

	result(): ProtocolResult<number> {
		return this.#failure ?? protocolSuccess(this.#offset);
	}

	#ensure(length: number): boolean {
		if (this.#failure) return false;
		if (!Number.isSafeInteger(length) || length < 0) {
			this.#failure = protocolFailure(
				ProtocolErrorCode.arithmeticOverflow,
				'Binary field length is not a non-negative safe integer',
			);
			return false;
		}
		if (length > this.#bytes.byteLength - this.#offset) {
			this.#failure = protocolFailure(
				ProtocolErrorCode.truncated,
				`Binary record ended at byte ${this.#offset} before ${length} bytes could be read`,
			);
			return false;
		}
		return true;
	}
}

export class BinaryWriter {
	readonly #bytes: Uint8Array;
	readonly #view: DataView;
	#offset = 0;

	constructor(length: number) {
		this.#bytes = new Uint8Array(length);
		this.#view = new DataView(this.#bytes.buffer);
	}

	get offset(): number {
		return this.#offset;
	}

	bytes(): Uint8Array {
		if (this.#offset !== this.#bytes.byteLength) {
			throw new Error(
				`Binary writer invariant failed: wrote ${this.#offset} of ${this.#bytes.byteLength} bytes`,
			);
		}
		return this.#bytes;
	}

	writeBytes(value: Uint8Array): void {
		this.#bytes.set(value, this.#offset);
		this.#offset += value.byteLength;
	}

	writeUint8(value: number): void {
		this.#view.setUint8(this.#offset, value);
		this.#offset += 1;
	}

	writeUint16(value: number): void {
		this.#view.setUint16(this.#offset, value, true);
		this.#offset += 2;
	}

	writeUint32(value: number): void {
		this.#view.setUint32(this.#offset, value, true);
		this.#offset += 4;
	}

	writeUint64(value: bigint): void {
		this.#view.setBigUint64(this.#offset, value, true);
		this.#offset += 8;
	}

	writeFloat32(value: number): void {
		this.#view.setFloat32(this.#offset, value, true);
		this.#offset += 4;
	}
}

export function checkedAdd(...values: readonly number[]): ProtocolResult<number> {
	let total = 0;
	for (const value of values) {
		if (!Number.isSafeInteger(value) || value < 0 || total > Number.MAX_SAFE_INTEGER - value) {
			return protocolFailure(
				ProtocolErrorCode.arithmeticOverflow,
				'Binary record size exceeds safe integer arithmetic',
			);
		}
		total += value;
	}
	return protocolSuccess(total);
}

export function checkedMultiply(left: number, right: number): ProtocolResult<number> {
	if (
		!Number.isSafeInteger(left) ||
		!Number.isSafeInteger(right) ||
		left < 0 ||
		right < 0 ||
		(left !== 0 && right > Math.floor(Number.MAX_SAFE_INTEGER / left))
	) {
		return protocolFailure(
			ProtocolErrorCode.arithmeticOverflow,
			'Binary record size exceeds safe integer arithmetic',
		);
	}
	return protocolSuccess(left * right);
}

export function isSharedArrayBuffer(buffer: ArrayBufferLike): boolean {
	if (sharedArrayBufferByteLengthGetter === undefined) return false;
	try {
		Reflect.apply(sharedArrayBufferByteLengthGetter, buffer, []);
		return true;
	} catch {
		return false;
	}
}

export function copyBytesToFixedArrayBuffer(
	bytes: Uint8Array,
	maximumBytes: number,
	recordName: string,
): ProtocolResult<Uint8Array> {
	try {
		const byteLength = bytes.byteLength;
		if (byteLength > maximumBytes) {
			return protocolFailure(ProtocolErrorCode.limitExceeded, `${recordName} exceeds its limit`);
		}
		const source = new Uint8Array(bytes.buffer, bytes.byteOffset, byteLength);
		const copy = new Uint8Array(byteLength);
		copy.set(source);
		return protocolSuccess(copy);
	} catch {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`${recordName} could not be copied into stable storage`,
		);
	}
}
