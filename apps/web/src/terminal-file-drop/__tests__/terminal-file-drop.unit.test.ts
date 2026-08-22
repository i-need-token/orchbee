// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';

import {
	containsTerminalFileDrop,
	createTerminalFileDropCapability,
	extractTerminalFileDropFiles,
	MAXIMUM_TERMINAL_FILE_DROP_FILES,
	MAXIMUM_TERMINAL_FILE_DROP_INPUT_CHARACTERS,
} from '../terminal-file-drop';

type TerminalFileDropPathResolver = Parameters<typeof createTerminalFileDropCapability>[0];

describe('terminal file drop transfer', () => {
	it('recognizes a protected native file drag without reading unavailable files', () => {
		const getAsFile = vi.fn<() => File | null>(() => null);
		const transfer = dataTransfer([], [{ getAsFile, kind: 'file' }], ['Files']);

		expect(containsTerminalFileDrop(transfer)).toBe(true);
		expect(getAsFile).not.toHaveBeenCalled();
	});

	it('prefers the ordered FileList and falls back to file-kind drop items', () => {
		const first = file('first');
		const second = file('second');
		const ignoredItem = vi.fn<() => File | null>(() => file('ignored'));

		expect(
			extractTerminalFileDropFiles(
				dataTransfer([first, second], [{ getAsFile: ignoredItem, kind: 'file' }], ['Files']),
			),
		).toEqual([first, second]);
		expect(ignoredItem).not.toHaveBeenCalled();

		const directory = file('ghostty');
		expect(
			extractTerminalFileDropFiles(
				dataTransfer(
					[],
					[
						{ getAsFile: () => null, kind: 'string' },
						{ getAsFile: () => directory, kind: 'file' },
						{ getAsFile: () => null, kind: 'file' },
					],
					['Files'],
				),
			),
		).toEqual([directory]);
	});

	it('does not claim an ordinary text drag', () => {
		expect(containsTerminalFileDrop(dataTransfer([], [], ['text/plain']))).toBe(false);
		expect(extractTerminalFileDropFiles(dataTransfer([], [], ['text/plain']))).toEqual([]);
	});
});

describe('terminal file drop preparation', () => {
	it('preserves order and applies Ghostty live-buffer escaping to each POSIX path', async () => {
		const files = [file('first'), file('second'), file('third'), file('fourth')];
		const resolver = pathResolver('darwin', [
			'/tmp/plain.txt',
			'/tmp/space name.txt',
			"/tmp/$(echo 'hi').txt",
			'/tmp/你好 🚀.txt',
		]);

		await expect(prepare(files, resolver)).resolves.toEqual({
			kind: 'ready',
			paths: ['/tmp/plain.txt', '/tmp/space name.txt', "/tmp/$(echo 'hi').txt", '/tmp/你好 🚀.txt'],
			text: "/tmp/plain.txt /tmp/space\\ name.txt /tmp/\\$\\(echo\\ \\'hi\\'\\).txt /tmp/你好\\ 🚀.txt",
		});
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(1, files[0]);
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(2, files[1]);
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(3, files[2]);
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(4, files[3]);
	});

	it.each([
		{ expected: 'unsupported', paths: ['/tmp/file'], platform: 'win32' },
		{ expected: 'unsupported', paths: ['/tmp/file'], platform: 'freebsd' },
		{ expected: 'unavailable-path', paths: [''], platform: 'darwin' },
		{ expected: 'invalid-path', paths: ['relative/file'], platform: 'darwin' },
		{ expected: 'invalid-path', paths: ['/tmp/tab\tfile'], platform: 'darwin' },
		{ expected: 'invalid-path', paths: ['/tmp/line\nbreak'], platform: 'darwin' },
		{ expected: 'invalid-path', paths: ['/tmp/return\rfile'], platform: 'darwin' },
		{ expected: 'invalid-path', paths: ['/tmp/escape\u001bfile'], platform: 'darwin' },
	] as const)(
		'rejects $expected input atomically on $platform',
		async ({ expected, paths, platform }) => {
			await expect(
				prepare(
					paths.map((_, index) => file(`file-${index}`)),
					pathResolver(platform, paths),
				),
			).resolves.toEqual({ kind: 'rejected', reason: expected });
		},
	);

	it('rejects an empty or oversized drop before resolving any backing path', async () => {
		const resolver = pathResolver('darwin', []);

		await expect(prepare([], resolver)).resolves.toEqual({
			kind: 'rejected',
			reason: 'empty',
		});
		await expect(
			prepare(
				Array.from({ length: MAXIMUM_TERMINAL_FILE_DROP_FILES + 1 }, (_, index) =>
					file(`file-${index}`),
				),
				resolver,
			),
		).resolves.toEqual({ kind: 'rejected', reason: 'too-many-files' });
		expect(resolver.getPathForFile).not.toHaveBeenCalled();
	});

	it('admits a complete 4096-code-point input and rejects one more', async () => {
		const admittedPath = `/${'a'.repeat(MAXIMUM_TERMINAL_FILE_DROP_INPUT_CHARACTERS - 1)}`;
		const rejectedPath = `${admittedPath}b`;

		await expect(
			prepare([file('admitted')], pathResolver('darwin', [admittedPath])),
		).resolves.toMatchObject({ kind: 'ready', text: admittedPath });
		await expect(
			prepare([file('rejected')], pathResolver('darwin', [rejectedPath])),
		).resolves.toEqual({ kind: 'rejected', reason: 'input-too-large' });
	});

	it('turns a thrown or non-string backing-path result into one visible rejection', async () => {
		await expect(
			prepare([file('throw')], {
				getPathForFile: () => {
					throw new Error('private adapter error');
				},
				platform: 'darwin',
			}),
		).resolves.toEqual({ kind: 'rejected', reason: 'unavailable-path' });
		await expect(
			prepare([file('wrong-type')], {
				getPathForFile: () => 42,
				platform: 'darwin',
			}),
		).resolves.toEqual({ kind: 'rejected', reason: 'unavailable-path' });
	});
});

function file(name: string): File {
	return new File([], name);
}

function dataTransfer(
	files: readonly File[],
	items: readonly { getAsFile(): File | null; readonly kind: string }[],
	types: readonly string[],
): Pick<DataTransfer, 'files' | 'items' | 'types'> {
	return { files, items, types } as unknown as Pick<DataTransfer, 'files' | 'items' | 'types'>;
}

function prepare(files: readonly File[], resolver: TerminalFileDropPathResolver) {
	return createTerminalFileDropCapability(resolver).prepare(files);
}

function pathResolver(
	platform: unknown,
	paths: readonly unknown[],
): TerminalFileDropPathResolver & {
	readonly getPathForFile: ReturnType<typeof vi.fn<TerminalFileDropPathResolver['getPathForFile']>>;
} {
	let index = 0;
	return {
		getPathForFile: vi.fn<TerminalFileDropPathResolver['getPathForFile']>(() => paths[index++]),
		platform,
	};
}
