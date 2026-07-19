import { createOrchbeeIntl, type IntlShape, type OrchbeeLocale, webMessages } from '@orchbee/i18n';
import type {
	WorkbenchSession,
	WorkbenchSessionService,
	WorkbenchSessionSnapshot,
	WorkbenchTerminalChannel,
	WorkbenchTerminalSize,
} from '@orchbee/workbench';

class InMemoryTerminalChannel implements WorkbenchTerminalChannel {
	private readonly intl: IntlShape;
	private readonly listeners = new Set<(data: string) => void>();
	private transcript: string;
	private inputBuffer = '';
	private size: WorkbenchTerminalSize = { cols: 80, rows: 24 };

	constructor(introduction: string, intl: IntlShape) {
		this.intl = intl;
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
				this.emit(`${this.intl.formatMessage(webMessages.helpOutput)}\r\n`);
				break;
			case 'status':
				this.emit(
					`${this.intl.formatMessage(webMessages.statusOutput, {
						columnCount: this.size.cols,
						rowCount: this.size.rows,
					})}\r\n`,
				);
				break;
			default:
				this.emit(
					`${this.intl.formatMessage(webMessages.simulatedCommandOutput, { command })}\r\n`,
				);
		}
		this.emit('$ ');
	}

	private emit(data: string): void {
		this.transcript += data;
		for (const listener of this.listeners) listener(data);
	}
}

class InMemorySessionService implements WorkbenchSessionService {
	private readonly intl: IntlShape;
	private readonly listeners = new Set<() => void>();
	private sessionCount = 0;
	private sessions: readonly WorkbenchSession[];
	private snapshot: WorkbenchSessionSnapshot;

	constructor(locale: OrchbeeLocale) {
		this.intl = createOrchbeeIntl(locale);
		this.sessions = [
			this.createSessionRecord(
				this.intl.formatMessage(webMessages.localShellTitle),
				this.intl.formatMessage(webMessages.localShellIntroduction),
			),
			this.createSessionRecord(
				this.intl.formatMessage(webMessages.agentOutputTitle),
				this.intl.formatMessage(webMessages.agentOutputIntroduction),
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
			this.intl.formatMessage(webMessages.newSessionTitle, {
				sessionNumber: this.sessionCount + 1,
			}),
			this.intl.formatMessage(webMessages.newSessionIntroduction),
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
			terminal: new InMemoryTerminalChannel(introduction.replaceAll('\n', '\r\n'), this.intl),
		};
	}

	private publish(): void {
		this.snapshot = { sessions: this.sessions };
		for (const listener of this.listeners) listener();
	}
}

export function createInMemorySessionService(locale: OrchbeeLocale): WorkbenchSessionService {
	return new InMemorySessionService(locale);
}
