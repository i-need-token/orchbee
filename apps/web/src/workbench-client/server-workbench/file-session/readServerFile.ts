import type { FileContent, FileError, FileReadOptions, FileResult } from '../../../file-session';
import {
	FILES_OPERATIONS,
	FILES_READ_CHUNK,
	type FilesChunk,
	type FilesChunkEnd,
} from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';
import { CANCELLED, fileFailure, unansweredError, untilAborted } from './server-file-exchange';

/** What `fs.read` reports before any chunk. */
interface ReadFacts {
	readonly binary: boolean;
	readonly bom: boolean;
	readonly encoding: string | null;
	readonly mtimeMs: number;
	readonly size: number;
}

type ChunkAnswer =
	| { readonly chunk: FilesChunk; readonly kind: 'chunk' }
	| { readonly error: FileError; readonly kind: 'error' };

type ServerFileRead = FileReadOptions & { readonly path: string };

/** A text read's UTF-8 is at most three bytes for each byte of the file. */
const TEXT_GROWTH = 3;
// The text keeps a leading U+FEFF the Server decoded; the file's own byte order mark never arrives.
const utf8 = new TextDecoder('utf-8', { ignoreBOM: true });

/**
 * Reads one file over a lane of its own: the opening request and the first chunk go together, the
 * other chunks follow as the read's facts allow, and closing the lane at the end drops the transfer
 * and whatever it still had in flight.
 */
export async function readServerFile(
	channel: ServerFileChannel,
	workspaceId: string,
	request: ServerFileRead,
	signal: AbortSignal,
): Promise<FileResult<FileContent>> {
	const transfer = channel.nextTransfer();
	const opening = FILES_OPERATIONS['fs.read'].encode({
		encoding: request.encoding,
		forceText: request.forceText,
		limit: request.limit,
		mode: request.mode,
		op: 'fs.read',
		path: request.path,
		transfer: transfer.toString(),
		workspaceId,
	});
	if (opening === null) return fileFailure('invalid-request');
	const release = await channel.budget.enter('transfer', signal);
	if (release === null) return CANCELLED;
	let laneId: bigint;
	try {
		laneId = channel.connection.openLane();
	} catch (error) {
		release();
		return { error: unansweredError(error), kind: 'error' };
	}
	try {
		const read = new ChunkedRead({ channel, laneId, request, signal, transfer, workspaceId });
		return await untilAborted(read.run(opening), signal);
	} finally {
		channel.connection.closeLane(laneId);
		release();
	}
}

interface ChunkedReadOptions {
	readonly channel: ServerFileChannel;
	readonly laneId: bigint;
	readonly request: ServerFileRead;
	readonly signal: AbortSignal;
	readonly transfer: bigint;
	readonly workspaceId: string;
}

/**
 * The chunks of one read, consumed in offset order whatever order they arrive in. The read's facts
 * come first, so a chunk refused because the file is binary or failed to open says nothing, and a
 * chunk past the end is never consumed. Text is served in order and bytes in any order, and every
 * chunk but the last fills its length, so offsets are known before the chunks arrive.
 */
class ChunkedRead {
	readonly #answers = new Map<number, ChunkAnswer>();
	/** The offset up to which chunks are asked for before the end is known. */
	#ceiling = 0;
	#consumed = 0;
	readonly #data: Uint8Array[] = [];
	#done = false;
	#facts: FileResult<ReadFacts> | null = null;
	#inFlight = 0;
	/** The most output this read accepts before it counts the file as too large. */
	#limit = 0;
	#next = 0;
	readonly #options: ChunkedReadOptions;
	#resolve: (result: FileResult<FileContent>) => void = () => undefined;
	readonly #settled: Promise<FileResult<FileContent>>;

	constructor(options: ChunkedReadOptions) {
		this.#options = options;
		this.#settled = new Promise((resolve) => {
			this.#resolve = resolve;
		});
	}

	run(opening: Uint8Array): Promise<FileResult<FileContent>> {
		void this.#open(opening);
		this.#ask();
		return this.#settled;
	}

	async #open(opening: Uint8Array): Promise<void> {
		const { channel, laneId, request, signal } = this.#options;
		const facts = await channel.request(
			opening,
			(record) => FILES_OPERATIONS['fs.read'].decode(record),
			{ laneId, signal },
		);
		this.#facts = facts;
		if (facts.kind === 'ok') {
			const growth = request.mode === 'text' ? TEXT_GROWTH : 1;
			this.#limit = growth * facts.value.size + channel.chunkLength;
			// A text read asks at its size too, for the empty end of output that fills its last chunk.
			this.#ceiling = request.mode === 'text' ? facts.value.size : facts.value.size - 1;
		}
		this.#pump();
	}

	#ask(): void {
		const offset = this.#next;
		this.#next += this.#options.channel.chunkLength;
		this.#inFlight += 1;
		void this.#fetch(offset).then((answer) => {
			this.#inFlight -= 1;
			this.#answers.set(offset, answer);
			this.#pump();
		});
	}

	async #fetch(offset: number): Promise<ChunkAnswer> {
		const { channel, laneId, signal, transfer, workspaceId } = this.#options;
		const request = FILES_READ_CHUNK.encode({
			length: channel.chunkLength,
			offset: offset.toString(),
			op: 'fs.read-chunk',
			transfer: transfer.toString(),
			workspaceId,
		});
		if (request === null) return { error: { code: 'invalid-request' }, kind: 'error' };
		const release = await channel.budget.enter('chunk', signal);
		if (release === null) return { error: { code: 'cancelled' }, kind: 'error' };
		try {
			const answer = FILES_READ_CHUNK.decode(
				await channel.connection.exchange(request, { laneId }),
			);
			if (answer === null) return { error: { code: 'failed' }, kind: 'error' };
			channel.answered(answer.kind === 'error' ? answer.error : null);
			return answer;
		} catch (error) {
			return { error: unansweredError(error), kind: 'error' };
		} finally {
			release();
		}
	}

	#pump(): void {
		if (this.#done) return;
		if (this.#options.signal.aborted) {
			this.#end(CANCELLED);
			return;
		}
		const facts = this.#facts;
		if (facts === null) return;
		if (facts.kind === 'error') {
			this.#end(facts);
			return;
		}
		const { channel, request, transfer } = this.#options;
		if (request.mode === 'text' && facts.value.binary && request.forceText !== true) {
			this.#end({ kind: 'ok', value: { kind: 'binary', version: version(facts.value) } });
			return;
		}
		for (;;) {
			const answer = this.#answers.get(this.#consumed);
			if (answer === undefined) break;
			this.#answers.delete(this.#consumed);
			if (answer.kind === 'error') {
				this.#end({ error: answer.error, kind: 'error' });
				return;
			}
			const { chunk } = answer;
			if (chunk.transfer !== transfer || chunk.offset !== BigInt(this.#consumed)) {
				this.#end(fileFailure('failed'));
				return;
			}
			// A file that keeps growing while it is read must not grow the read without bound.
			if (this.#consumed + chunk.data.byteLength > this.#limit) {
				this.#end(fileFailure('too-large'));
				return;
			}
			this.#data.push(chunk.data);
			this.#consumed += chunk.data.byteLength;
			if (chunk.end !== null) {
				this.#end(this.#content(facts.value, chunk.end));
				return;
			}
			if (chunk.data.byteLength !== channel.chunkLength) {
				this.#end(fileFailure('failed'));
				return;
			}
		}
		if (this.#inFlight === 0) {
			// Every chunk asked for arrived without the end, so the output outgrew the file's size.
			if (this.#next > this.#limit) {
				this.#end(fileFailure('too-large'));
				return;
			}
			this.#ceiling = this.#limit;
		}
		while (this.#inFlight < channel.chunkWindow && this.#next <= this.#ceiling) this.#ask();
	}

	#content(facts: ReadFacts, end: FilesChunkEnd): FileResult<FileContent> {
		const bytes = new Uint8Array(this.#consumed);
		let offset = 0;
		for (const piece of this.#data) {
			bytes.set(piece, offset);
			offset += piece.byteLength;
		}
		if (this.#options.request.mode === 'bytes') {
			return {
				kind: 'ok',
				value: { bytes, changed: end.changed, kind: 'bytes', version: version(facts) },
			};
		}
		if (facts.encoding === null) return fileFailure('failed');
		return {
			kind: 'ok',
			value: {
				bom: facts.bom,
				changed: end.changed,
				encoding: facts.encoding,
				kind: 'text',
				lineEnding: end.lineEnding,
				text: utf8.decode(bytes),
				version: version(facts),
			},
		};
	}

	#end(result: FileResult<FileContent>): void {
		if (this.#done) return;
		this.#done = true;
		this.#resolve(result);
	}
}

function version(facts: ReadFacts): { readonly mtimeMs: number; readonly size: number } {
	return { mtimeMs: facts.mtimeMs, size: facts.size };
}
