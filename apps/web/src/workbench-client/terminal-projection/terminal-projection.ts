import type { TerminalGeometry } from '@orchbee/terminal-protocol';
import type {
	ProjectedAttachmentBootstrap,
	ProjectedTerminalCommand,
	TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

/**
 * `refused` means the request was turned down or never left; `unknown` means it may have taken
 * effect, so repeating it is the way to be sure.
 */
export type ProjectedTerminalTakeControlOutcome = 'acquired' | 'refused' | 'unknown';

export interface ProjectedTerminalTransport {
	close(): void;
	/** Declares whether projection payloads cross an external validation boundary. */
	readonly projectionBufferTrust: 'external' | 'local-host';
	send(command: ProjectedTerminalCommand): void;
	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void;
	/**
	 * Present where another Client can hold the terminal's input and size. Asks for both on this
	 * attachment at a person's request; a granted request arrives as the next capability revision
	 * before this settles, and no renderer command can make the request.
	 */
	takeControl?: (() => Promise<ProjectedTerminalTakeControlOutcome>) | undefined;
}

export interface TerminalProjection {
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly transport: ProjectedTerminalTransport;
}

export interface TerminalProjectionRequest {
	readonly result: Promise<TerminalProjection>;
	release(): void;
}

export interface TerminalProjectionClient {
	dispose(): void;
	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
	): TerminalProjectionRequest;
}
