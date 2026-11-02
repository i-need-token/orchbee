/**
 * A terminal program's bell and clipboard write requests, as the Host reports them to one viewer.
 * The renderer performs both effects itself, so a clipboard decision only answers the request it
 * names and asks nothing of the Server. Until it does, a later request is dropped.
 */
import {
	type DecodedProtocolEnvelope,
	MessageKind,
	type TerminalClipboardWriteRequest,
	TerminalBellSchema,
	TerminalClipboardWriteRequestSchema,
	decodeControlJson,
} from '@orchbee/terminal-protocol';
import type { ProjectedTerminalEvent } from '@orchbee/workbench-client-protocol';

export class ServerTerminalEffects {
	readonly #attachmentId: string;
	readonly #post: (event: ProjectedTerminalEvent) => void;
	#pendingClipboardWrite: TerminalClipboardWriteRequest | undefined;

	constructor(attachmentId: string, post: (event: ProjectedTerminalEvent) => void) {
		this.#attachmentId = attachmentId;
		this.#post = post;
	}

	/** Returns why the record is no effect of this viewer, or null once it has gone to the renderer. */
	receive(envelope: DecodedProtocolEnvelope): string | null {
		if (envelope.kind === MessageKind.terminalBell) {
			const event = decodeControlJson(envelope.payload, TerminalBellSchema);
			if (!event.ok) return event.detail;
			if (event.value.attachmentId !== this.#attachmentId)
				return 'A bell named another attachment.';
			this.#post({ event: event.value, kind: 'bell' });
			return null;
		}
		const event = decodeControlJson(envelope.payload, TerminalClipboardWriteRequestSchema);
		if (!event.ok) return event.detail;
		if (event.value.attachmentId !== this.#attachmentId) {
			return 'A clipboard write request named another attachment.';
		}
		if (this.#pendingClipboardWrite !== undefined) return null;
		// Before the post: a renderer without its own clipboard decides at once.
		this.#pendingClipboardWrite = event.value;
		this.#post({ event: event.value, kind: 'clipboard-write-request' });
		return null;
	}

	/** Whether the decision answers the request that is waiting for one. */
	decide(decision: TerminalClipboardWriteRequest): boolean {
		const pending = this.#pendingClipboardWrite;
		if (
			pending === undefined ||
			pending.attachmentId !== decision.attachmentId ||
			pending.sequence !== decision.sequence ||
			pending.text !== decision.text
		) {
			return false;
		}
		this.#pendingClipboardWrite = undefined;
		return true;
	}
}
