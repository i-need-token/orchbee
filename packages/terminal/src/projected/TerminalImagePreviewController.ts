import type { CanonicalUint64Text } from '@orchbee/terminal-protocol';

import type { TerminalOpenTarget } from '../terminal-contracts';

const TERMINAL_IMAGE_PREVIEW_DWELL_MS = 300;

export type TerminalImagePreviewMediaType =
	| 'image/png'
	| 'image/jpeg'
	| 'image/gif'
	| 'image/webp'
	| 'image/avif'
	| 'image/bmp';

export interface TerminalImagePreviewAnchor {
	readonly height: number;
	readonly left: number;
	readonly surfaceHeight: number;
	readonly surfaceWidth: number;
	readonly top: number;
	readonly width: number;
}

export interface TerminalImagePreviewMembership {
	readonly anchor: TerminalImagePreviewAnchor;
	readonly context: {
		readonly column: number;
		readonly generation: string;
		readonly linkEpoch: string;
		readonly revision: string;
		readonly row: string;
	};
	readonly target: Extract<TerminalOpenTarget, { readonly kind: 'file' }>;
}

export type TerminalImagePreviewPresentation =
	| { readonly kind: 'hidden' }
	| {
			readonly anchor: TerminalImagePreviewAnchor;
			readonly content: ArrayBuffer;
			readonly kind: 'visible';
			readonly mediaType: TerminalImagePreviewMediaType;
			readonly path: string;
	  };

export type TerminalImagePreviewSnapshot =
	| { readonly phase: 'idle' }
	| { readonly phase: 'disposed' }
	| {
			readonly membership: TerminalImagePreviewMembership;
			readonly phase: 'waiting' | 'unavailable' | 'visible';
	  }
	| {
			readonly membership: TerminalImagePreviewMembership;
			readonly phase: 'loading';
			readonly requestSequence: CanonicalUint64Text;
	  };

interface TerminalImagePreviewControllerOptions {
	readonly cancel: (requestSequence: CanonicalUint64Text) => void;
	readonly present: (presentation: TerminalImagePreviewPresentation) => void;
	readonly request: (membership: TerminalImagePreviewMembership) => CanonicalUint64Text;
}

export class TerminalImagePreviewController {
	readonly #cancel: TerminalImagePreviewControllerOptions['cancel'];
	readonly #present: TerminalImagePreviewControllerOptions['present'];
	readonly #request: TerminalImagePreviewControllerOptions['request'];
	#snapshot: TerminalImagePreviewSnapshot = { phase: 'idle' };
	#timer: ReturnType<typeof setTimeout> | undefined;

	constructor(options: TerminalImagePreviewControllerOptions) {
		this.#cancel = options.cancel;
		this.#present = options.present;
		this.#request = options.request;
	}

	get snapshot(): TerminalImagePreviewSnapshot {
		return this.#snapshot;
	}

	hover(membership: TerminalImagePreviewMembership): void {
		const current = this.#snapshot;
		if (current.phase === 'disposed') return;
		if (
			current.phase !== 'idle' &&
			terminalImagePreviewMembershipsAreEqual(current.membership, membership)
		) {
			return;
		}

		const waiting = { membership, phase: 'waiting' } as const;
		this.#retireCurrentMembership(waiting);
		if (this.#snapshot !== waiting) return;
		this.#timer = globalThis.setTimeout(() => {
			if (this.#snapshot !== waiting) return;
			this.#timer = undefined;

			let requestSequence: CanonicalUint64Text;
			try {
				requestSequence = this.#request(membership);
			} catch {
				if (this.#snapshot === waiting) {
					this.#snapshot = { membership, phase: 'unavailable' };
				}
				return;
			}

			if (this.#snapshot !== waiting) {
				this.#cancelRequest(requestSequence);
				return;
			}
			this.#snapshot = { membership, phase: 'loading', requestSequence };
		}, TERMINAL_IMAGE_PREVIEW_DWELL_MS);
	}

	complete(
		requestSequence: CanonicalUint64Text,
		preview: {
			readonly content: ArrayBuffer;
			readonly mediaType: TerminalImagePreviewMediaType;
		},
	): void {
		if (this.#snapshot.phase !== 'loading' || this.#snapshot.requestSequence !== requestSequence) {
			return;
		}

		const membership = this.#snapshot.membership;
		this.#snapshot = { membership, phase: 'visible' };
		this.#present({
			anchor: membership.anchor,
			content: preview.content,
			kind: 'visible',
			mediaType: preview.mediaType,
			path: membership.target.path,
		});
	}

	unavailable(requestSequence: CanonicalUint64Text): void {
		if (this.#snapshot.phase !== 'loading' || this.#snapshot.requestSequence !== requestSequence) {
			return;
		}
		this.#snapshot = { membership: this.#snapshot.membership, phase: 'unavailable' };
	}

	retire(): void {
		if (this.#snapshot.phase === 'disposed') return;
		this.#retireCurrentMembership({ phase: 'idle' });
	}

	dispose(): void {
		if (this.#snapshot.phase === 'disposed') return;
		this.#retireCurrentMembership({ phase: 'disposed' });
	}

	#retireCurrentMembership(nextSnapshot: TerminalImagePreviewSnapshot): void {
		const previous = this.#snapshot;
		this.#snapshot = nextSnapshot;
		if (this.#timer !== undefined) {
			globalThis.clearTimeout(this.#timer);
			this.#timer = undefined;
		}
		if (previous.phase === 'loading') {
			this.#cancelRequest(previous.requestSequence);
		}
		if (previous.phase === 'visible') {
			this.#present({ kind: 'hidden' });
		}
	}

	#cancelRequest(requestSequence: CanonicalUint64Text): void {
		try {
			this.#cancel(requestSequence);
		} catch {
			// Cancellation is advisory; request identity still prevents stale publication.
		}
	}
}

function terminalImagePreviewMembershipsAreEqual(
	left: TerminalImagePreviewMembership,
	right: TerminalImagePreviewMembership,
): boolean {
	return (
		left.target.path === right.target.path &&
		left.target.line === right.target.line &&
		left.target.column === right.target.column &&
		left.context.column === right.context.column &&
		left.context.generation === right.context.generation &&
		left.context.linkEpoch === right.context.linkEpoch &&
		left.context.revision === right.context.revision &&
		left.context.row === right.context.row &&
		left.anchor.height === right.anchor.height &&
		left.anchor.left === right.anchor.left &&
		left.anchor.surfaceHeight === right.anchor.surfaceHeight &&
		left.anchor.surfaceWidth === right.anchor.surfaceWidth &&
		left.anchor.top === right.anchor.top &&
		left.anchor.width === right.anchor.width
	);
}
