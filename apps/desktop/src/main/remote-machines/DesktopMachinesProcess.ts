import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import type { MessagePortMain } from 'electron';
import { z } from 'zod';
import type { DesktopDiagnosticRecorder } from '../diagnostics';

/** The longest record a page may post, in UTF-16 code units. */
const PAGE_RECORD_UNITS = 65_536;
/** The longest line the process writes; a longer one is discarded up to its LF. */
const PROCESS_LINE_BYTES = 8 * 1024 * 1024;
/** How long the process may take to leave the hub once its standard input closes. */
const STOP_MILLISECONDS = 1_500;

const ProcessLineSchema = z.discriminatedUnion('kind', [
	z.strictObject({ available: z.boolean(), kind: z.literal('started'), version: z.literal(1) }),
	z.strictObject({
		document: z.string(),
		kind: z.literal('page'),
		record: z.string(),
		version: z.literal(1),
	}),
]);

export interface DesktopMachinesAttachment {
	close(): void;
}

export interface DesktopMachinesProcessOptions {
	readonly clientEnvironment: NodeJS.ProcessEnv;
	readonly clientExecutable: string;
	readonly recordEvent: DesktopDiagnosticRecorder;
}

/**
 * Supervises `orchbee --desktop-machines-v1` for Desktop's life and carries the page's records to
 * it and its records to the attached document's port, reading only the envelope. One that says the
 * host lacks saved machines, or exits, is not started again.
 */
export class DesktopMachinesProcess {
	#attached: { readonly document: string; readonly port: MessagePortMain } | undefined;
	#child: ChildProcessWithoutNullStreams | undefined;
	readonly #options: DesktopMachinesProcessOptions;

	constructor(options: DesktopMachinesProcessOptions) {
		this.#options = options;
	}

	start(): void {
		const options = this.#options;
		const child = spawn(options.clientExecutable, ['--desktop-machines-v1'], {
			env: options.clientEnvironment,
			stdio: ['pipe', 'pipe', 'pipe'],
			windowsHide: true,
		});
		this.#child = child;
		child.stdin.on('error', () => undefined);
		// Standard error carries the process's diagnostics, which name no fact Main records.
		child.stderr.resume();
		readLines(
			child.stdout,
			(line) => this.#receive(line),
			() => options.recordEvent('warning', 'desktop-machines-line-dropped'),
		);
		const ended = (): void => {
			if (this.#child !== child) return;
			this.#child = undefined;
			this.#attached?.port.close();
			this.#attached = undefined;
		};
		child.on('error', () => {
			options.recordEvent('error', 'desktop-machines-start-failed');
			ended();
		});
		child.on('exit', ended);
	}

	/** Whether Desktop has a window. */
	setWindow(exists: boolean): void {
		this.#write({ exists, kind: 'window' });
	}

	/** Connects `document`'s port, replacing the attached document; without a process, closes it. */
	attach(document: string, port: MessagePortMain): DesktopMachinesAttachment {
		if (this.#child === undefined) {
			port.close();
			return { close: () => undefined };
		}
		this.#attached?.port.close();
		this.#attached = { document, port };
		this.#write({ document, kind: 'attach' });
		port.on('message', (event) => {
			if (this.#attached?.port !== port) return;
			const record: unknown = event.data;
			if (typeof record !== 'string' || record.length > PAGE_RECORD_UNITS) {
				this.#options.recordEvent('warning', 'desktop-machines-record-dropped');
				return;
			}
			this.#write({ document, kind: 'page', record });
		});
		let closed = false;
		const close = (): void => {
			if (closed) return;
			closed = true;
			if (this.#attached?.port === port) {
				this.#attached = undefined;
				this.#write({ document, kind: 'detach' });
			}
			port.close();
		};
		port.on('close', close);
		port.start();
		return { close };
	}

	/** Closes the process's standard input and ends one that has not left in time. */
	async stop(): Promise<void> {
		const child = this.#child;
		if (child === undefined) return;
		const exited = new Promise<void>((resolve) => child.once('exit', () => resolve()));
		child.stdin.end();
		const timer = new Promise<'late'>((resolve) =>
			setTimeout(() => resolve('late'), STOP_MILLISECONDS).unref(),
		);
		if ((await Promise.race([exited, timer])) === 'late') child.kill();
	}

	#receive(text: string): void {
		let value: unknown;
		try {
			value = JSON.parse(text);
		} catch {
			value = undefined;
		}
		const line = ProcessLineSchema.safeParse(value);
		if (!line.success) {
			this.#options.recordEvent('warning', 'desktop-machines-line-dropped');
			return;
		}
		if (line.data.kind === 'started') {
			// The process exits itself; until then no document is attached.
			if (!line.data.available) this.#attached?.port.close();
			return;
		}
		const attached = this.#attached;
		if (attached?.document === line.data.document) attached.port.postMessage(line.data.record);
	}

	#write(line: Readonly<Record<string, unknown>>): void {
		const child = this.#child;
		if (child === undefined || !child.stdin.writable) return;
		child.stdin.write(`${JSON.stringify({ ...line, version: 1 })}\n`);
	}
}

/** Calls `line` with each LF-ended UTF-8 line of `stream`, and `dropped` for one over the bound. */
function readLines(
	stream: NodeJS.ReadableStream,
	line: (text: string) => void,
	dropped: () => void,
): void {
	let pending: Buffer[] = [];
	let length = 0;
	let discarding = false;
	stream.on('data', (chunk: Buffer) => {
		let rest = chunk;
		for (let end = rest.indexOf(0x0a); end !== -1; end = rest.indexOf(0x0a)) {
			const head = rest.subarray(0, end);
			rest = rest.subarray(end + 1);
			if (!discarding && length + head.length <= PROCESS_LINE_BYTES) {
				line(Buffer.concat([...pending, head]).toString('utf8'));
			} else {
				dropped();
			}
			pending = [];
			length = 0;
			discarding = false;
		}
		if (discarding) return;
		if (length + rest.length > PROCESS_LINE_BYTES) {
			pending = [];
			length = 0;
			discarding = true;
			return;
		}
		pending.push(rest);
		length += rest.length;
	});
}
