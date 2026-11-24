import { spawn } from 'node:child_process';
import type { Readable } from 'node:stream';
import type { DesktopMachineSessionReply } from '@orchbee/desktop-client-protocol';
import { MessageChannelMain, type MessagePortMain } from 'electron';
import { z } from 'zod';

import type { DesktopDiagnosticRecorder } from '../diagnostics';
import { ServerConnectionRelay, type ServerConnectProcess } from '../server-connection';

/** How long a relay may take to say whether the machine's Session admitted it. */
const FIRST_LINE_MILLISECONDS = 30_000;
/** The longest first line read; a relay's announcement is far shorter. */
const FIRST_LINE_BYTES = 64 * 1024;

const CapabilityNameSchema = z.string().regex(/^[a-z0-9-]{1,64}$/u);

/** The relay's first line, `machines.md#desktop-session-relay`. */
const RelayLineSchema = z.discriminatedUnion('outcome', [
	z.strictObject({
		capabilities: z.array(CapabilityNameSchema).max(256),
		outcome: z.literal('admitted'),
		version: z.literal(1),
	}),
	z.strictObject({
		outcome: z.literal('refused'),
		reason: z.enum([
			'client-too-old',
			'development-build-mismatch',
			'hub-unavailable',
			'link-lost',
			'not-found',
			'other-environment',
			'protocol',
			'session-too-old',
			'transient',
		]),
		version: z.literal(1),
	}),
]);

/** An answer without the document and request Main adds to it. */
type Outcome = DistributiveOmit<DesktopMachineSessionReply, 'document' | 'request'>;
type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never;

/** Answers one request: with the relay's port when admitted, or why there is none. */
export type DesktopMachineSessionAnswer = (outcome: Outcome, port?: MessagePortMain) => void;

interface DesktopMachineSessionsOptions {
	readonly clientEnvironment: NodeJS.ProcessEnv;
	readonly clientExecutable: string;
	readonly recordEvent: DesktopDiagnosticRecorder;
}

/** One relay: starting until its first line is read, then carrying the page's port. */
type Held =
	| { readonly kind: 'starting'; readonly process: ServerConnectProcess }
	| {
			readonly kind: 'relaying';
			readonly process: ServerConnectProcess;
			readonly relay: ServerConnectionRelay;
	  };

/**
 * The remote Session connections of one document: one `orchbee --machine-session` relay per saved
 * machine whose Workspaces the page shows, as many as it shows. Main reads only each relay's first
 * line; after it the existing bounded relay carries the bytes. Ending a relay never stops the
 * remote Session.
 */
export class DesktopMachineSessions {
	readonly #held = new Map<string, Held>();
	readonly #options: DesktopMachineSessionsOptions;
	#closed = false;

	constructor(options: DesktopMachineSessionsOptions) {
		this.#options = options;
	}

	/**
	 * Starts `machine`'s relay and answers once: with the relay's port, or why there is none. A
	 * machine whose connection the document already holds or is starting is refused `unavailable`.
	 */
	open(machine: string, answer: DesktopMachineSessionAnswer): void {
		if (this.#closed || this.#held.has(machine)) {
			answer({ outcome: 'refused', reason: 'unavailable' });
			return;
		}
		const options = this.#options;
		const process: ServerConnectProcess = spawn(
			options.clientExecutable,
			['--machine-session', machine],
			{ env: options.clientEnvironment, stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true },
		);
		const held: Held = { kind: 'starting', process };
		this.#held.set(machine, held);
		process.stdin.on('error', () => undefined);
		process.once('error', () => undefined);
		// A relay's connection lasts as long as its process; one still starting answers first.
		process.once('exit', () => this.#release(machine, process));
		void readFirstLine(process.stdout, FIRST_LINE_MILLISECONDS).then((read) => {
			if (this.#held.get(machine) !== held) {
				process.kill();
				return;
			}
			const line = read.kind === 'line' ? parseLine(read.text) : undefined;
			if (line?.outcome !== 'admitted') {
				this.#held.delete(machine);
				process.kill();
				if (line?.outcome === 'refused') {
					answer({ outcome: 'refused', reason: line.reason });
					return;
				}
				// Only a relay outside its contract is Main's to record.
				const reason = read.kind === 'timeout' ? 'timeout' : 'protocol';
				options.recordEvent('warning', 'machine-session-refused', { reason });
				answer({ outcome: 'refused', reason });
				return;
			}
			const channel = new MessageChannelMain();
			const relay = new ServerConnectionRelay(process, channel.port1, options.recordEvent);
			// The page closing its end frees the machine at once, before the relay's process exits.
			channel.port1.on('close', () => this.#release(machine, process));
			this.#held.set(machine, { kind: 'relaying', process, relay });
			// The first line's reader paused the output; the relay now reads the rest.
			process.stdout.resume();
			answer(
				{ bootstrap: { machine, negotiatedCapabilities: line.capabilities }, outcome: 'admitted' },
				channel.port2,
			);
		});
	}

	/** The document ended: every relay ends, and one still starting answers nothing. */
	close(): void {
		this.#closed = true;
		for (const held of this.#held.values()) {
			if (held.kind === 'relaying') held.relay.close();
			else held.process.kill();
		}
		this.#held.clear();
	}

	#release(machine: string, process: ServerConnectProcess): void {
		const held = this.#held.get(machine);
		if (held?.kind === 'relaying' && held.process === process) this.#held.delete(machine);
	}
}

function parseLine(text: string): z.infer<typeof RelayLineSchema> | undefined {
	try {
		const parsed = RelayLineSchema.safeParse(JSON.parse(text));
		return parsed.success ? parsed.data : undefined;
	} catch {
		return undefined;
	}
}

type FirstLine =
	| { readonly kind: 'line'; readonly text: string }
	| { readonly kind: 'ended' }
	| { readonly kind: 'timeout' };

/**
 * Reads `stream` up to its first LF within `milliseconds`, then pauses it and puts back what
 * followed, so the bytes after the line reach whoever reads next.
 */
function readFirstLine(stream: Readable, milliseconds: number): Promise<FirstLine> {
	return new Promise((resolve) => {
		const chunks: Buffer[] = [];
		let length = 0;
		const finish = (result: FirstLine): void => {
			clearTimeout(timer);
			stream.off('data', data);
			stream.off('end', ended);
			stream.off('error', ended);
			resolve(result);
		};
		const data = (chunk: Buffer): void => {
			const end = chunk.indexOf(0x0a);
			if (end === -1) {
				chunks.push(chunk);
				length += chunk.byteLength;
				if (length > FIRST_LINE_BYTES) finish({ kind: 'ended' });
				return;
			}
			stream.pause();
			const rest = chunk.subarray(end + 1);
			if (rest.byteLength > 0) stream.unshift(rest);
			finish({
				kind: 'line',
				text: Buffer.concat([...chunks, chunk.subarray(0, end)]).toString('utf8'),
			});
		};
		const ended = (): void => finish({ kind: 'ended' });
		const timer = setTimeout(() => finish({ kind: 'timeout' }), milliseconds);
		stream.on('data', data);
		stream.once('end', ended);
		stream.once('error', ended);
	});
}
