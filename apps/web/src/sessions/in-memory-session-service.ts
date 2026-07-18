import type {
	WorkbenchSession,
	WorkbenchSessionService,
	WorkbenchSessionSnapshot,
	WorkbenchTerminalChannel,
	WorkbenchTerminalSize,
} from '@orchbee/workbench';

class InMemoryTerminalChannel implements WorkbenchTerminalChannel {
	private readonly listeners = new Set<(data: string) => void>();
	private transcript: string;
	private inputBuffer = '';
	private size: WorkbenchTerminalSize = { cols: 80, rows: 24 };

	constructor(introduction: string) {
		this.transcript = `${introduction}\r\n\r\n$ `;
	}

	subscribeOutput(listener: (data: string) => void): () => void {
		this.listeners.add(listener);
		listener(this.transcript);
		return () => this.listeners.delete(listener);
	}

	write(data: string): void {
		if (data.startsWith('\u001b')) return;
		for (const character of data) {
			if (character === '\r') {
				this.runCommand();
				continue;
			}
			if (character === '\n') continue;
			if (character === '\u007f') {
				const characters = Array.from(this.inputBuffer);
				if (characters.length > 0) {
					characters.pop();
					this.inputBuffer = characters.join('');
					this.emit('\b \b');
				}
				continue;
			}
			this.inputBuffer += character;
			this.emit(character);
		}
	}

	resize(size: WorkbenchTerminalSize): void {
		this.size = size;
	}

	private runCommand(): void {
		const command = this.inputBuffer.trim();
		this.inputBuffer = '';
		this.emit('\r\n');

		switch (command) {
			case '':
				break;
			case 'clear':
				this.emit('\u001b[2J\u001b[H');
				break;
			case 'help':
				this.emit('Available commands: help, status, clear\r\n');
				break;
			case 'status':
				this.emit(`In-memory session · ${this.size.cols} columns × ${this.size.rows} rows\r\n`);
				break;
			default:
				this.emit(`Simulated command: ${command}\r\n`);
		}
		this.emit('$ ');
	}

	private emit(data: string): void {
		this.transcript += data;
		for (const listener of this.listeners) listener(data);
	}
}

class InMemorySessionService implements WorkbenchSessionService {
	private readonly listeners = new Set<() => void>();
	private sessionCount = 0;
	private sessions: readonly WorkbenchSession[];
	private snapshot: WorkbenchSessionSnapshot;

	constructor() {
		this.sessions = [
			this.createSessionRecord(
				'Local shell',
				'Orchbee Workbench\r\nThis terminal is backed by the in-memory session adapter. Type help to begin.',
			),
			this.createSessionRecord(
				'Agent output',
				'Codex agent · ready\r\nWaiting for a task in the first Workbench vertical slice.',
			),
		];
		this.snapshot = { sessions: this.sessions };
	}

	getSnapshot(): WorkbenchSessionSnapshot {
		return this.snapshot;
	}

	subscribe(listener: () => void): () => void {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	}

	createSession(): string {
		const session = this.createSessionRecord(
			`Session ${this.sessionCount + 1}`,
			'New in-memory terminal session.',
		);
		this.sessions = [...this.sessions, session];
		this.publish();
		return session.id;
	}

	closeSession(sessionId: string): void {
		const nextSessions = this.sessions.filter((session) => session.id !== sessionId);
		if (nextSessions.length === this.sessions.length) return;
		this.sessions = nextSessions;
		this.publish();
	}

	private createSessionRecord(title: string, introduction: string): WorkbenchSession {
		this.sessionCount += 1;
		return {
			id: `memory-session-${this.sessionCount}`,
			title,
			status: 'running',
			terminal: new InMemoryTerminalChannel(introduction),
		};
	}

	private publish(): void {
		this.snapshot = { sessions: this.sessions };
		for (const listener of this.listeners) listener();
	}
}

export function createInMemorySessionService(): WorkbenchSessionService {
	return new InMemorySessionService();
}
