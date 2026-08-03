import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type {
	ProjectedAttachmentBootstrap,
	ProjectedTerminalCommand,
} from '@orchbee/workbench-client-protocol';

export interface ProjectedTerminalTransport {
	close(): void;
	send(command: ProjectedTerminalCommand): void;
	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void;
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
	open(presentationId: TerminalPresentationId): TerminalProjectionRequest;
}
