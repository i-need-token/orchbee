export type WorkbenchSessionStatus = 'starting' | 'running' | 'disconnected' | 'exited';

export interface WorkbenchTerminalSize {
	readonly cols: number;
	readonly rows: number;
}

export interface WorkbenchTerminalChannel {
	subscribeOutput(listener: (data: string) => void): () => void;
	write(data: string): void;
	resize(size: WorkbenchTerminalSize): void;
}

export interface WorkbenchSession {
	readonly id: string;
	readonly title: string;
	readonly status: WorkbenchSessionStatus;
	readonly terminal: WorkbenchTerminalChannel;
}

export interface WorkbenchSessionSnapshot {
	readonly sessions: readonly WorkbenchSession[];
}

export interface WorkbenchSessionService {
	getSnapshot(): WorkbenchSessionSnapshot;
	subscribe(listener: () => void): () => void;
	createSession(): string;
	closeSession(sessionId: string): void;
}
