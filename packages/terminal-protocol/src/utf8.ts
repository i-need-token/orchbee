function isContinuationByte(value: number | undefined): boolean {
	return value !== undefined && value >= 0x80 && value <= 0xbf;
}

export function utf8ByteLength(value: string): number {
	let byteLength = 0;
	for (let index = 0; index < value.length; index += 1) {
		const codeUnit = value.charCodeAt(index);
		if (codeUnit <= 0x7f) byteLength += 1;
		else if (codeUnit <= 0x7ff) byteLength += 2;
		else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
			const nextCodeUnit = value.charCodeAt(index + 1);
			if (nextCodeUnit >= 0xdc00 && nextCodeUnit <= 0xdfff) {
				byteLength += 4;
				index += 1;
			} else byteLength += 3;
		} else byteLength += 3;
	}
	return byteLength;
}

/** Validates Unicode scalar UTF-8 without materializing a UTF-16 string. */
export function isValidUtf8(bytes: Uint8Array): boolean {
	let offset = 0;
	while (offset < bytes.byteLength) {
		const first = bytes[offset];
		if (first === undefined) return false;
		if (first <= 0x7f) {
			offset += 1;
			continue;
		}

		const second = bytes[offset + 1];
		if (first >= 0xc2 && first <= 0xdf) {
			if (!isContinuationByte(second)) return false;
			offset += 2;
			continue;
		}

		const third = bytes[offset + 2];
		if (first === 0xe0) {
			if (second === undefined || second < 0xa0 || second > 0xbf || !isContinuationByte(third)) {
				return false;
			}
			offset += 3;
			continue;
		}
		if ((first >= 0xe1 && first <= 0xec) || (first >= 0xee && first <= 0xef)) {
			if (!isContinuationByte(second) || !isContinuationByte(third)) return false;
			offset += 3;
			continue;
		}
		if (first === 0xed) {
			if (second === undefined || second < 0x80 || second > 0x9f || !isContinuationByte(third)) {
				return false;
			}
			offset += 3;
			continue;
		}

		const fourth = bytes[offset + 3];
		if (first === 0xf0) {
			if (
				second === undefined ||
				second < 0x90 ||
				second > 0xbf ||
				!isContinuationByte(third) ||
				!isContinuationByte(fourth)
			) {
				return false;
			}
			offset += 4;
			continue;
		}
		if (first >= 0xf1 && first <= 0xf3) {
			if (
				!isContinuationByte(second) ||
				!isContinuationByte(third) ||
				!isContinuationByte(fourth)
			) {
				return false;
			}
			offset += 4;
			continue;
		}
		if (first === 0xf4) {
			if (
				second === undefined ||
				second < 0x80 ||
				second > 0x8f ||
				!isContinuationByte(third) ||
				!isContinuationByte(fourth)
			) {
				return false;
			}
			offset += 4;
			continue;
		}
		return false;
	}
	return true;
}
