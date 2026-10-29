import { createHash } from 'node:crypto';
import { mkdtemp, readdir, rm, writeFile, type FileHandle } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	terminalPresentationId,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import { MAX_TERMINAL_ARCHIVE_BYTES, type TerminalArchive } from '../terminal-archive-document';

const ARCHIVE_FILE_SUFFIX = '.obta';
const temporaryDirectories: string[] = [];

type ArchiveReadStep = Error | Uint8Array | number;

afterEach(async () => {
	vi.doUnmock('node:fs/promises');
	vi.doUnmock('../terminal-archive-document');
	vi.resetModules();
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map(async (directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('FileTerminalArchiveStore boundary', () => {
	it('treats a missing local archive as an empty stopped terminal', async () => {
		const directory = await createTemporaryDirectory('orchbee-missing-terminal-archive-');
		const store = new FileTerminalArchiveStore(directory);
		expect(await store.load(terminalPresentationId('missing-presentation'))).toBeNull();
	});

	it('rejects an oversized load before payload read and closes its opened handle', async () => {
		const directory = await createTemporaryDirectory('orchbee-oversized-terminal-archive-');
		const presentationId = terminalPresentationId('oversized-presentation');
		const archivePath = await createArchivePlaceholder(directory, presentationId);
		const handle = createArchiveHandle({
			readSteps: [new Error('Payload read must not run after an oversized stat')],
			statSize: MAX_TERMINAL_ARCHIVE_BYTES + 1,
		});
		const mocked = await importMockedStore();
		mocked.open.mockResolvedValue(handle.fileHandle);
		mocked.readFile.mockResolvedValue(Buffer.from([0xff]));
		mocked.decodeTerminalArchive.mockReturnValue(invalidArchive());
		const store = new mocked.FileTerminalArchiveStore(directory);

		expect(await store.load(presentationId)).toBeNull();
		expect(mocked.unlink).toHaveBeenCalledWith(archivePath);
		expect({
			closeCalls: handle.close.mock.calls.length,
			handleReadCalls: handle.read.mock.calls.length,
			openCalls: mocked.open.mock.calls.length,
			pathReadFileCalls: mocked.readFile.mock.calls.length,
			statCalls: handle.stat.mock.calls.length,
			unlinkCalls: mocked.unlink.mock.calls.length,
		}).toEqual({
			closeCalls: 1,
			handleReadCalls: 0,
			openCalls: 1,
			pathReadFileCalls: 0,
			statCalls: 1,
			unlinkCalls: 1,
		});
		expect(mocked.open).toHaveBeenCalledWith(archivePath, 'r');
	});

	it('reconciles an oversized archive beside one valid retained archive through closed handles', async () => {
		const directory = await createTemporaryDirectory('orchbee-reconciled-terminal-archive-');
		const oversizedId = terminalPresentationId('oversized-reconciled-presentation');
		const retainedId = terminalPresentationId('valid-reconciled-presentation');
		const oversizedPath = await createArchivePlaceholder(directory, oversizedId);
		const retainedPath = await createArchivePlaceholder(directory, retainedId);
		const oversizedHandle = createArchiveHandle({
			readSteps: [new Error('Payload read must not run after an oversized stat')],
			statSize: MAX_TERMINAL_ARCHIVE_BYTES + 1,
		});
		const retainedHandle = createArchiveHandle({
			readSteps: [new Uint8Array([0x01]), 0],
			statSize: 1,
		});
		const mocked = await importMockedStore();
		mocked.open.mockImplementation(async (filePath) => {
			if (filePath === oversizedPath) return oversizedHandle.fileHandle;
			if (filePath === retainedPath) return retainedHandle.fileHandle;
			throw new Error(`Unexpected archive open: ${filePath}`);
		});
		mocked.readFile.mockImplementation(async (filePath) => {
			if (filePath === oversizedPath) return Buffer.from([0xff]);
			if (filePath === retainedPath) return Buffer.from([0x01]);
			throw new Error(`Unexpected archive path read: ${filePath}`);
		});
		mocked.decodeTerminalArchive.mockImplementation((encoded) =>
			encoded[0] === 0x01 ? validArchive(retainedId) : invalidArchive(),
		);
		const store = new mocked.FileTerminalArchiveStore(directory);

		await store.reconcile(new Set([retainedId]));

		expect(await readdir(directory)).toEqual([path.basename(retainedPath)]);
		expect(mocked.unlink).toHaveBeenCalledWith(oversizedPath);
		expect({
			decodeCalls: mocked.decodeTerminalArchive.mock.calls.length,
			openCalls: mocked.open.mock.calls.length,
			oversizedCloseCalls: oversizedHandle.close.mock.calls.length,
			oversizedReadCalls: oversizedHandle.read.mock.calls.length,
			pathReadFileCalls: mocked.readFile.mock.calls.length,
			retainedCloseCalls: retainedHandle.close.mock.calls.length,
			retainedReadCalls: retainedHandle.read.mock.calls.length,
		}).toEqual({
			decodeCalls: 1,
			openCalls: 2,
			oversizedCloseCalls: 1,
			oversizedReadCalls: 0,
			pathReadFileCalls: 0,
			retainedCloseCalls: 1,
			retainedReadCalls: 2,
		});
	});

	it('detects growth through one buffer capped at the archive limit plus one byte', async () => {
		const directory = await createTemporaryDirectory('orchbee-grown-terminal-archive-');
		const presentationId = terminalPresentationId('grown-presentation');
		const archivePath = await createArchivePlaceholder(directory, presentationId);
		const handle = createArchiveHandle({
			readSteps: [MAX_TERMINAL_ARCHIVE_BYTES + 1],
			statSize: 1,
		});
		const mocked = await importMockedStore();
		mocked.open.mockResolvedValue(handle.fileHandle);
		mocked.readFile.mockResolvedValue(Buffer.alloc(MAX_TERMINAL_ARCHIVE_BYTES + 1, 0x61));
		mocked.decodeTerminalArchive.mockReturnValue(invalidArchive());
		const store = new mocked.FileTerminalArchiveStore(directory);

		expect(await store.load(presentationId)).toBeNull();
		expect(mocked.unlink).toHaveBeenCalledWith(archivePath);
		expect({
			closeCalls: handle.close.mock.calls.length,
			decodeCalls: mocked.decodeTerminalArchive.mock.calls.length,
			handleReadCalls: handle.read.mock.calls.length,
			largestReadBufferCapacity: largestReadBufferCapacity(handle.read.mock.calls),
			largestRequestedRead: largestRequestedRead(handle.read.mock.calls),
			openCalls: mocked.open.mock.calls.length,
			pathReadFileCalls: mocked.readFile.mock.calls.length,
			statCalls: handle.stat.mock.calls.length,
		}).toEqual({
			closeCalls: 1,
			decodeCalls: 0,
			handleReadCalls: 1,
			largestReadBufferCapacity: MAX_TERMINAL_ARCHIVE_BYTES + 1,
			largestRequestedRead: MAX_TERMINAL_ARCHIVE_BYTES + 1,
			openCalls: 1,
			pathReadFileCalls: 0,
			statCalls: 1,
		});
	});

	it('admits an exact-limit payload to decoding only after its handle closes', async () => {
		const directory = await createTemporaryDirectory('orchbee-boundary-terminal-archive-');
		const presentationId = terminalPresentationId('boundary-presentation');
		await createArchivePlaceholder(directory, presentationId);
		const handle = createArchiveHandle({
			readSteps: [MAX_TERMINAL_ARCHIVE_BYTES, 0],
			statSize: MAX_TERMINAL_ARCHIVE_BYTES,
		});
		const mocked = await importMockedStore();
		let closeCallsObservedByDecoder = -1;
		let decodedByteLength = -1;
		mocked.open.mockResolvedValue(handle.fileHandle);
		mocked.readFile.mockResolvedValue(Buffer.alloc(MAX_TERMINAL_ARCHIVE_BYTES, 0x61));
		mocked.decodeTerminalArchive.mockImplementation((encoded) => {
			closeCallsObservedByDecoder = handle.close.mock.calls.length;
			decodedByteLength = encoded.byteLength;
			return invalidArchive();
		});
		const store = new mocked.FileTerminalArchiveStore(directory);

		expect(await store.load(presentationId)).toBeNull();
		expect({
			closeCalls: handle.close.mock.calls.length,
			closeCallsObservedByDecoder,
			decodeCalls: mocked.decodeTerminalArchive.mock.calls.length,
			decodedByteLength,
			handleReadCalls: handle.read.mock.calls.length,
			largestRequestedRead: largestRequestedRead(handle.read.mock.calls),
			pathReadFileCalls: mocked.readFile.mock.calls.length,
		}).toEqual({
			closeCalls: 1,
			closeCallsObservedByDecoder: 1,
			decodeCalls: 1,
			decodedByteLength: MAX_TERMINAL_ARCHIVE_BYTES,
			handleReadCalls: 2,
			largestRequestedRead: MAX_TERMINAL_ARCHIVE_BYTES + 1,
			pathReadFileCalls: 0,
		});
	});

	it('passes only partially read bytes to decoding after EOF without a zero-fill tail', async () => {
		const directory = await createTemporaryDirectory('orchbee-partial-terminal-archive-');
		const presentationId = terminalPresentationId('partial-presentation');
		await createArchivePlaceholder(directory, presentationId);
		const handle = createArchiveHandle({
			readSteps: [new Uint8Array([1, 2]), new Uint8Array([3]), 0],
			statSize: 6,
		});
		const mocked = await importMockedStore();
		let closeCallsObservedByDecoder = -1;
		mocked.open.mockResolvedValue(handle.fileHandle);
		mocked.readFile.mockResolvedValue(Buffer.from([1, 2, 3]));
		mocked.decodeTerminalArchive.mockImplementation(() => {
			closeCallsObservedByDecoder = handle.close.mock.calls.length;
			return invalidArchive();
		});
		const store = new mocked.FileTerminalArchiveStore(directory);

		expect(await store.load(presentationId)).toBeNull();
		const decoded = mocked.decodeTerminalArchive.mock.calls[0]?.[0];
		expect(decoded?.byteLength).toBe(3);
		expect(decoded === undefined ? [] : [...decoded]).toEqual([1, 2, 3]);
		expect(closeCallsObservedByDecoder).toBe(1);
		expect(handle.read.mock.calls.map((call) => call[2])).toEqual([
			MAX_TERMINAL_ARCHIVE_BYTES + 1,
			MAX_TERMINAL_ARCHIVE_BYTES - 1,
			MAX_TERMINAL_ARCHIVE_BYTES - 2,
		]);
		expect(mocked.readFile).not.toHaveBeenCalled();
		expect(handle.close).toHaveBeenCalledTimes(1);
	});

	it('closes an opened handle when archive stat fails without unlinking the artifact', async () => {
		const directory = await createTemporaryDirectory('orchbee-stat-failure-terminal-archive-');
		const presentationId = terminalPresentationId('stat-failure-presentation');
		await createArchivePlaceholder(directory, presentationId);
		const statError = new Error('synthetic archive stat failure');
		const handle = createArchiveHandle({ statError, statSize: 0 });
		const mocked = await importMockedStore();
		mocked.open.mockResolvedValue(handle.fileHandle);
		mocked.readFile.mockRejectedValue(statError);
		const store = new mocked.FileTerminalArchiveStore(directory);

		expect(await store.load(presentationId)).toBeNull();
		expect({
			closeCalls: handle.close.mock.calls.length,
			decodeCalls: mocked.decodeTerminalArchive.mock.calls.length,
			handleReadCalls: handle.read.mock.calls.length,
			openCalls: mocked.open.mock.calls.length,
			pathReadFileCalls: mocked.readFile.mock.calls.length,
			statCalls: handle.stat.mock.calls.length,
			unlinkCalls: mocked.unlink.mock.calls.length,
		}).toEqual({
			closeCalls: 1,
			decodeCalls: 0,
			handleReadCalls: 0,
			openCalls: 1,
			pathReadFileCalls: 0,
			statCalls: 1,
			unlinkCalls: 0,
		});
	});

	it('closes an opened handle when a payload read fails without unlinking the artifact', async () => {
		const directory = await createTemporaryDirectory('orchbee-read-failure-terminal-archive-');
		const presentationId = terminalPresentationId('read-failure-presentation');
		await createArchivePlaceholder(directory, presentationId);
		const readError = new Error('synthetic archive read failure');
		const handle = createArchiveHandle({ readSteps: [readError], statSize: 1 });
		const mocked = await importMockedStore();
		mocked.open.mockResolvedValue(handle.fileHandle);
		mocked.readFile.mockRejectedValue(readError);
		const store = new mocked.FileTerminalArchiveStore(directory);

		expect(await store.load(presentationId)).toBeNull();
		expect({
			closeCalls: handle.close.mock.calls.length,
			decodeCalls: mocked.decodeTerminalArchive.mock.calls.length,
			handleReadCalls: handle.read.mock.calls.length,
			openCalls: mocked.open.mock.calls.length,
			pathReadFileCalls: mocked.readFile.mock.calls.length,
			statCalls: handle.stat.mock.calls.length,
			unlinkCalls: mocked.unlink.mock.calls.length,
		}).toEqual({
			closeCalls: 1,
			decodeCalls: 0,
			handleReadCalls: 1,
			openCalls: 1,
			pathReadFileCalls: 0,
			statCalls: 1,
			unlinkCalls: 0,
		});
	});
});

async function importMockedStore() {
	vi.resetModules();
	const actualFileSystem = await import('node:fs/promises');
	const actualArchiveDocument = await import('../terminal-archive-document');
	const open = vi.fn<(filePath: string, flags: string) => Promise<FileHandle>>();
	const readFile = vi.fn<(filePath: string) => Promise<Buffer>>();
	const unlink = vi.fn<(filePath: string) => Promise<void>>();
	unlink.mockImplementation(async (filePath) => actualFileSystem.unlink(filePath));
	const decodeTerminalArchive = vi.fn<typeof actualArchiveDocument.decodeTerminalArchive>();
	vi.doMock('node:fs/promises', () => ({
		...actualFileSystem,
		open,
		readFile,
		unlink,
	}));
	vi.doMock('../terminal-archive-document', () => ({
		...actualArchiveDocument,
		decodeTerminalArchive,
	}));
	const mockedStore = await import('../FileTerminalArchiveStore');
	return {
		FileTerminalArchiveStore: mockedStore.FileTerminalArchiveStore,
		decodeTerminalArchive,
		open,
		readFile,
		unlink,
	};
}

function createArchiveHandle(options: {
	readonly readSteps?: readonly ArchiveReadStep[];
	readonly statError?: Error;
	readonly statSize: number;
}) {
	let readIndex = 0;
	const close = vi.fn<() => Promise<void>>().mockResolvedValue();
	const read =
		vi.fn<
			(
				buffer: Uint8Array,
				offset: number,
				length: number,
			) => Promise<{ readonly buffer: Uint8Array; readonly bytesRead: number }>
		>();
	read.mockImplementation(async (buffer, offset, length) => {
		const step = options.readSteps?.[readIndex] ?? 0;
		readIndex += 1;
		if (step instanceof Error) throw step;
		const bytesRead = typeof step === 'number' ? step : step.byteLength;
		if (bytesRead > length) throw new Error('Synthetic read exceeds the requested capacity');
		if (typeof step === 'number') {
			buffer.fill(0x61, offset, offset + bytesRead);
		} else {
			buffer.set(step, offset);
		}
		return { buffer, bytesRead };
	});
	const stat = vi.fn<() => Promise<{ readonly size: number }>>();
	if (options.statError === undefined) {
		stat.mockResolvedValue({ size: options.statSize });
	} else {
		stat.mockRejectedValue(options.statError);
	}
	return {
		close,
		fileHandle: { close, read, stat } as unknown as FileHandle,
		read,
		stat,
	};
}

function invalidArchive() {
	return { detail: 'synthetic invalid archive', ok: false } as const;
}

function validArchive(presentationId: TerminalPresentationId) {
	return {
		ok: true,
		value: { presentationId } as TerminalArchive,
	} as const;
}

function largestRequestedRead(
	calls: readonly (readonly [buffer: Uint8Array, offset: number, length: number])[],
): number {
	return calls.reduce((largest, call) => Math.max(largest, call[2]), 0);
}

function largestReadBufferCapacity(
	calls: readonly (readonly [buffer: Uint8Array, offset: number, length: number])[],
): number {
	return calls.reduce((largest, call) => Math.max(largest, call[0].byteLength), 0);
}

async function createTemporaryDirectory(prefix: string): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), prefix));
	temporaryDirectories.push(directory);
	return directory;
}

async function createArchivePlaceholder(
	directory: string,
	presentationId: TerminalPresentationId,
): Promise<string> {
	const filePath = terminalArchivePath(directory, presentationId);
	await writeFile(filePath, new Uint8Array([0]));
	return filePath;
}

function terminalArchivePath(directory: string, presentationId: TerminalPresentationId): string {
	const digest = createHash('sha256').update(presentationId).digest('hex');
	return path.join(directory, `${digest}${ARCHIVE_FILE_SUFFIX}`);
}
