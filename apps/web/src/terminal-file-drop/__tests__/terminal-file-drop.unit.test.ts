// @vitest-environment jsdom

import { describe, expect, it, vi } from 'vitest';

import {
	createTerminalFileDropCapability,
	MAXIMUM_TERMINAL_FILE_DROP_FILES,
	MAXIMUM_TERMINAL_FILE_DROP_PREVIEW_CHARACTERS,
} from '../terminal-file-drop';

type TerminalFileDropPathResolver = Parameters<typeof createTerminalFileDropCapability>[0];

describe('terminal file drop preparation', () => {
	it('preserves order and quotes each POSIX path as exactly one shell word', async () => {
		const files = [file('first'), file('second'), file('third')];
		const resolver = pathResolver('darwin', [
			'/tmp/space name.txt',
			"/tmp/quote's.txt",
			'/tmp/你好 🚀.txt',
		]);

		await expect(prepare(files, resolver)).resolves.toEqual({
			containsNewline: false,
			kind: 'ready',
			paths: ['/tmp/space name.txt', "/tmp/quote's.txt", '/tmp/你好 🚀.txt'],
			text: "'/tmp/space name.txt' '/tmp/quote'\\''s.txt' '/tmp/你好 🚀.txt'",
		});
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(1, files[0]);
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(2, files[1]);
		expect(resolver.getPathForFile).toHaveBeenNthCalledWith(3, files[2]);
	});

	it('retains a newline and tab inside the quoted argument without adding Enter', async () => {
		const path = '/tmp/line\nbreak\tfile';

		await expect(
			prepare([file('control-visible')], pathResolver('linux', [path])),
		).resolves.toEqual({
			containsNewline: true,
			kind: 'ready',
			paths: [path],
			text: `'${path}'`,
		});
	});

	it.each([
		{ expected: 'unsupported', paths: ['/tmp/file'], platform: 'win32' },
		{ expected: 'unsupported', paths: ['/tmp/file'], platform: 'freebsd' },
		{ expected: 'unavailable-path', paths: [''], platform: 'darwin' },
		{ expected: 'invalid-path', paths: ['relative/file'], platform: 'darwin' },
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

	it('admits a completely visible 4096-code-point preview and rejects one more', async () => {
		const admittedPath = `/${'a'.repeat(MAXIMUM_TERMINAL_FILE_DROP_PREVIEW_CHARACTERS - 3)}`;
		const rejectedPath = `${admittedPath}b`;

		await expect(
			prepare([file('admitted')], pathResolver('darwin', [admittedPath])),
		).resolves.toMatchObject({ kind: 'ready', text: `'${admittedPath}'` });
		await expect(
			prepare([file('rejected')], pathResolver('darwin', [rejectedPath])),
		).resolves.toEqual({ kind: 'rejected', reason: 'preview-too-large' });
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
