import { constants } from 'node:fs';
import { open, stat } from 'node:fs/promises';
import path from 'node:path';

import { MAX_PROJECTED_TERMINAL_IMAGE_PREVIEW_BYTES } from '@orchbee/workbench-client-protocol';
import { resolveTerminalLocalPath } from './resolveTerminalLocalPath';

type TerminalImagePreviewMediaType =
	| 'image/png'
	| 'image/jpeg'
	| 'image/gif'
	| 'image/webp'
	| 'image/avif'
	| 'image/bmp';

export interface TerminalImagePreviewContent {
	readonly content: ArrayBuffer;
	readonly mediaType: TerminalImagePreviewMediaType;
}

const previewMediaTypeByExtension: ReadonlyMap<string, TerminalImagePreviewMediaType> = new Map([
	['.avif', 'image/avif'],
	['.bmp', 'image/bmp'],
	['.gif', 'image/gif'],
	['.jpeg', 'image/jpeg'],
	['.jpg', 'image/jpeg'],
	['.png', 'image/png'],
	['.webp', 'image/webp'],
]);

export async function openTerminalLocalPath(
	target: string,
	openPath: (resolvedPath: string) => Promise<string>,
): Promise<void> {
	const resolvedPath = resolveTerminalLocalPath(target);
	const metadata = await stat(resolvedPath);
	if (!metadata.isFile() && !metadata.isDirectory()) {
		throw new Error('The terminal file target is neither a regular file nor a directory.');
	}
	const errorMessage = await openPath(resolvedPath);
	if (errorMessage !== '') throw new Error(errorMessage);
}

export async function readTerminalImagePreview(
	target: string,
	signal: AbortSignal,
): Promise<TerminalImagePreviewContent> {
	const resolvedPath = resolveTerminalLocalPath(target);
	const expectedMediaType = previewMediaTypeByExtension.get(
		path.extname(resolvedPath).toLowerCase(),
	);
	if (expectedMediaType === undefined) throw new Error('The file type is not previewable.');
	throwIfAborted(signal);

	const handle = await open(resolvedPath, constants.O_RDONLY | constants.O_NONBLOCK);
	try {
		throwIfAborted(signal);
		const metadata = await handle.stat();
		if (
			!metadata.isFile() ||
			!Number.isSafeInteger(metadata.size) ||
			metadata.size <= 0 ||
			metadata.size > MAX_PROJECTED_TERMINAL_IMAGE_PREVIEW_BYTES
		) {
			throw new Error('The image preview file is invalid or exceeds the size limit.');
		}

		const bytes = new Uint8Array(metadata.size);
		let offset = 0;
		while (offset < bytes.byteLength) {
			throwIfAborted(signal);
			const result = await handle.read(bytes, offset, bytes.byteLength - offset, offset);
			if (result.bytesRead === 0) throw new Error('The image preview file changed while reading.');
			offset += result.bytesRead;
		}
		throwIfAborted(signal);
		const mediaType = detectImageMediaType(bytes);
		if (mediaType !== expectedMediaType) {
			throw new Error('The image preview content does not match its file type.');
		}
		const content = new ArrayBuffer(bytes.byteLength);
		new Uint8Array(content).set(bytes);
		return { content, mediaType };
	} finally {
		await handle.close();
	}
}

function detectImageMediaType(bytes: Uint8Array): TerminalImagePreviewMediaType | undefined {
	if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) return 'image/png';
	if (startsWith(bytes, [0xff, 0xd8, 0xff])) return 'image/jpeg';
	if (startsWith(bytes, [0x47, 0x49, 0x46, 0x38, 0x37, 0x61])) return 'image/gif';
	if (startsWith(bytes, [0x47, 0x49, 0x46, 0x38, 0x39, 0x61])) return 'image/gif';
	if (
		startsWith(bytes, [0x52, 0x49, 0x46, 0x46]) &&
		matchesAt(bytes, 8, [0x57, 0x45, 0x42, 0x50])
	) {
		return 'image/webp';
	}
	if (startsWith(bytes, [0x42, 0x4d])) return 'image/bmp';
	if (matchesAt(bytes, 4, [0x66, 0x74, 0x79, 0x70]) && containsAvifBrand(bytes)) {
		return 'image/avif';
	}
	return undefined;
}

function containsAvifBrand(bytes: Uint8Array): boolean {
	const maximumOffset = Math.min(bytes.byteLength - 4, 60);
	for (let offset = 8; offset <= maximumOffset; offset += 4) {
		if (
			matchesAt(bytes, offset, [0x61, 0x76, 0x69, 0x66]) ||
			matchesAt(bytes, offset, [0x61, 0x76, 0x69, 0x73])
		) {
			return true;
		}
	}
	return false;
}

function startsWith(bytes: Uint8Array, signature: readonly number[]): boolean {
	return matchesAt(bytes, 0, signature);
}

function matchesAt(bytes: Uint8Array, offset: number, signature: readonly number[]): boolean {
	if (offset + signature.length > bytes.byteLength) return false;
	return signature.every((value, index) => bytes[offset + index] === value);
}

function throwIfAborted(signal: AbortSignal): void {
	if (signal.aborted) throw signal.reason;
}
