import type { FileError, FileResult } from '../../../file-session';
import { FILES_OPERATIONS, type FilesReply } from '../../server-connection';
import { ServerFileBudget } from './ServerFileBudget';
import { ServerFileLoads } from './ServerFileLoads';
import {
	CANCELLED,
	fileFailure,
	type ServerFileConnection,
	unansweredError,
} from './server-file-exchange';

/** A session's part in what happens to the connection's file service. */
export interface ServerFileServiceFollower {
	/** The connection ended, so nothing it held remains and no request will be answered. */
	ended(): void;
	/** The service restarted, which dropped every grant it held for the connection. */
	lost(): void;
	/** The service answered; `regained` after a restart dropped the grants. */
	served(regained: boolean): void;
}

interface ServerFileRequestOptions {
	readonly laneId?: bigint | undefined;
	/** Gives up waiting for room; a request already sent still runs. */
	readonly signal?: AbortSignal | undefined;
}

/** Bytes each chunk asks for: 128 KiB from this computer's Server, 32 KiB from another's. */
const LOCAL_CHUNK_BYTES = 131_072;
const REMOTE_CHUNK_BYTES = 32_768;
/** The data a connection keeps in flight: 2 MiB from this computer's Server, 256 KiB remotely. */
const LOCAL_WINDOW_BYTES = 2_097_152;
const REMOTE_WINDOW_BYTES = 262_144;

const channels = new WeakMap<ServerFileConnection, ServerFileChannel>();

/**
 * What every File Bar session of one Server connection shares: room for streams and reads, the
 * folders its listings loaded, transfer IDs, and what the file service's answers say about it. A
 * saved machine's runtime is created again each time its surface shows, over the same connection,
 * so this lives with the connection rather than with one runtime's sessions.
 */
export class ServerFileChannel {
	readonly budget: ServerFileBudget;
	readonly chunkLength: number;
	/** Chunks the connection keeps in flight, which together fill its data window. */
	readonly chunkWindow: number;
	readonly connection: ServerFileConnection;
	readonly loads: ServerFileLoads;
	#ended = false;
	readonly #followers = new Set<ServerFileServiceFollower>();
	/** A restart dropped the grants, and the service has not answered since. */
	#lost = false;
	#restarts = 0;
	#transfer = 0n;

	private constructor(connection: ServerFileConnection, remote: boolean) {
		this.chunkLength = remote ? REMOTE_CHUNK_BYTES : LOCAL_CHUNK_BYTES;
		this.chunkWindow = (remote ? REMOTE_WINDOW_BYTES : LOCAL_WINDOW_BYTES) / this.chunkLength;
		this.budget = new ServerFileBudget(this.chunkWindow);
		this.connection = connection;
		this.loads = new ServerFileLoads(async (workspaceId, paths) => {
			// Whatever it answers, the folders are no longer this Client's to hold.
			await this.request(
				FILES_OPERATIONS['fs.release'].encode({ op: 'fs.release', paths: [...paths], workspaceId }),
				(record) => FILES_OPERATIONS['fs.release'].decode(record),
			);
		});
		connection.onEnded(() => {
			this.#ended = true;
			for (const follower of this.#followers) follower.ended();
		});
	}

	/** The connection's channel, created with its first session. */
	static of(connection: ServerFileConnection, remote: boolean): ServerFileChannel {
		let channel = channels.get(connection);
		if (channel === undefined) {
			channel = new ServerFileChannel(connection, remote);
			channels.set(connection, channel);
		}
		return channel;
	}

	/** Follows the service until the returned function is called; an ended connection says so now. */
	follow(follower: ServerFileServiceFollower): () => void {
		if (this.#ended) {
			follower.ended();
			return () => undefined;
		}
		this.#followers.add(follower);
		return () => this.#followers.delete(follower);
	}

	/** How many restarts the service's answers have reported. */
	get restarts(): number {
		return this.#restarts;
	}

	/** A transfer ID no other transfer of the connection uses. */
	nextTransfer(): bigint {
		this.#transfer += 1n;
		return this.#transfer;
	}

	/**
	 * Runs one request once a stream is free and reads its Result; it never rejects. A null record
	 * broke the contract's rules or bounds before it was sent.
	 */
	async request<Value>(
		record: Uint8Array | null,
		decode: (result: Uint8Array) => FilesReply<Value> | null,
		options: ServerFileRequestOptions = {},
	): Promise<FileResult<Value>> {
		if (record === null) return fileFailure('invalid-request');
		if (this.#ended) return fileFailure('remote-disconnected');
		const release = await this.budget.enter('stream', options.signal);
		if (release === null) return CANCELLED;
		try {
			let result: Uint8Array;
			try {
				result = await this.connection.exchange(
					record,
					options.laneId === undefined ? {} : { laneId: options.laneId },
				);
			} catch (error) {
				return { error: unansweredError(error), kind: 'error' };
			}
			const reply = decode(result);
			// A Result this Client cannot read tells it nothing it could act on.
			if (reply === null) return fileFailure('failed');
			this.answered(reply.kind === 'error' ? reply.error : null);
			return reply;
		} finally {
			release();
		}
	}

	/**
	 * Learns from a Result or a snapshot of the file service: null for an answer that is not a
	 * failure. A failure other than `unavailable` is an answer too; `restarting` and
	 * `crashed-repeatedly` say the process that held the grants is gone.
	 */
	answered(error: FileError | null): void {
		if (error?.code === 'unavailable') {
			if (error.reason !== 'restarting' && error.reason !== 'crashed-repeatedly') return;
			if (this.#lost) return;
			this.#lost = true;
			this.#restarts += 1;
			for (const follower of this.#followers) follower.lost();
			return;
		}
		const regained = this.#lost;
		this.#lost = false;
		for (const follower of this.#followers) follower.served(regained);
	}
}
