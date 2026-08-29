import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { MAX_PROJECTED_TERMINAL_IMAGE_PREVIEW_BYTES } from '@orchbee/workbench-client-protocol';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { openTerminalLocalPath, readTerminalImagePreview } from '../terminal-local-file-effects';

describe('terminal local file effects', () => {
	let temporaryDirectory = '';

	beforeEach(async () => {
		temporaryDirectory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-file-effects-'));
	});

	afterEach(async () => {
		await rm(temporaryDirectory, { force: true, recursive: true });
	});

	it('opens only an existing regular file or directory', async () => {
		const filePath = path.join(temporaryDirectory, 'output.png');
		const directoryPath = path.join(temporaryDirectory, 'folder');
		await writeFile(filePath, pngBytes());
		await mkdir(directoryPath);
		const openPath = vi.fn<(resolvedPath: string) => Promise<string>>().mockResolvedValue('');

		await openTerminalLocalPath(filePath, openPath);
		await openTerminalLocalPath(directoryPath, openPath);

		expect(openPath.mock.calls).toEqual([[filePath], [directoryPath]]);
		await expect(
			openTerminalLocalPath(path.join(temporaryDirectory, 'missing.png'), openPath),
		).rejects.toThrow(/ENOENT/u);
		openPath.mockResolvedValueOnce('No default application');
		await expect(openTerminalLocalPath(filePath, openPath)).rejects.toThrow(
			'No default application',
		);
	});

	it.each([
		['output.png', pngBytes(), 'image/png'],
		['output.jpg', new Uint8Array([0xff, 0xd8, 0xff, 0xdb]), 'image/jpeg'],
		['output.gif', new TextEncoder().encode('GIF89a'), 'image/gif'],
		[
			'output.webp',
			new Uint8Array([0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50]),
			'image/webp',
		],
		['output.bmp', new Uint8Array([0x42, 0x4d, 0, 0]), 'image/bmp'],
		[
			'output.avif',
			new Uint8Array([0, 0, 0, 24, 0x66, 0x74, 0x79, 0x70, 0x61, 0x76, 0x69, 0x66]),
			'image/avif',
		],
	] as const)('validates %s by magic bytes', async (fileName, bytes, mediaType) => {
		const filePath = path.join(temporaryDirectory, fileName);
		await writeFile(filePath, bytes);

		const preview = await readTerminalImagePreview(filePath, new AbortController().signal);

		expect(preview.mediaType).toBe(mediaType);
		expect(new Uint8Array(preview.content)).toEqual(bytes);
	});

	it('rejects mismatched, excluded, oversized, and non-regular preview targets', async () => {
		const mismatchPath = path.join(temporaryDirectory, 'mismatch.png');
		const svgPath = path.join(temporaryDirectory, 'vector.svg');
		const oversizedPath = path.join(temporaryDirectory, 'oversized.png');
		await writeFile(mismatchPath, new Uint8Array([0xff, 0xd8, 0xff]));
		await writeFile(svgPath, '<svg/>');
		await writeFile(oversizedPath, new Uint8Array(MAX_PROJECTED_TERMINAL_IMAGE_PREVIEW_BYTES + 1));

		await expect(
			readTerminalImagePreview(mismatchPath, new AbortController().signal),
		).rejects.toThrow('does not match');
		await expect(readTerminalImagePreview(svgPath, new AbortController().signal)).rejects.toThrow(
			'not previewable',
		);
		await expect(
			readTerminalImagePreview(oversizedPath, new AbortController().signal),
		).rejects.toThrow('exceeds the size limit');
		await expect(
			readTerminalImagePreview(temporaryDirectory, new AbortController().signal),
		).rejects.toThrow('not previewable');
	});

	it('honors cancellation before reading file content', async () => {
		const filePath = path.join(temporaryDirectory, 'output.png');
		await writeFile(filePath, pngBytes());
		const controller = new AbortController();
		controller.abort(new Error('cancelled'));

		await expect(readTerminalImagePreview(filePath, controller.signal)).rejects.toThrow(
			'cancelled',
		);
	});
});

function pngBytes(): Uint8Array {
	return new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
}
