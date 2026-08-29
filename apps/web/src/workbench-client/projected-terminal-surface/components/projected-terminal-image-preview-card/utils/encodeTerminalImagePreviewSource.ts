import type { TerminalImagePreviewMediaType } from '@orchbee/terminal';

export function encodeTerminalImagePreviewSource(
	content: ArrayBuffer,
	mediaType: TerminalImagePreviewMediaType,
): string {
	const bytes = new Uint8Array(content);
	const encodedChunks: string[] = [];
	const chunkByteLength = 24 * 1024;
	for (let offset = 0; offset < bytes.byteLength; offset += chunkByteLength) {
		const end = Math.min(offset + chunkByteLength, bytes.byteLength);
		let binary = '';
		for (let index = offset; index < end; index += 1) {
			binary += String.fromCharCode(bytes[index] ?? 0);
		}
		encodedChunks.push(globalThis.btoa(binary));
	}
	return `data:${mediaType};base64,${encodedChunks.join('')}`;
}
