import type { IDisposable } from '../EventEmitter';

export interface TerminalKeyEventContext {
	readonly alternateScreen: boolean;
	readonly hasSelection: boolean;
	readonly readOnly: boolean;
}

export interface TerminalKeyEventHandler {
	readonly id: string;
	readonly priority?: number;
	/** Ownership scope used for bulk removal; both scopes participate in arbitration. */
	readonly scope?: 'surface' | 'application';
	readonly when?: (context: TerminalKeyEventContext) => boolean;
	handle(event: KeyboardEvent, context: TerminalKeyEventContext): boolean;
}

/** Ordered synchronous key arbitration before terminal protocol encoding. */
export class KeyEventRegistry {
	private readonly handlers: TerminalKeyEventHandler[] = [];

	register(handler: TerminalKeyEventHandler): IDisposable {
		if (this.handlers.some((candidate) => candidate.id === handler.id)) {
			throw new Error(`Key event handler already registered: ${handler.id}`);
		}
		let index = 0;
		while (
			index < this.handlers.length &&
			(this.handlers[index]?.priority ?? 0) >= (handler.priority ?? 0)
		) {
			index++;
		}
		this.handlers.splice(index, 0, handler);
		return {
			dispose: () => {
				const current = this.handlers.indexOf(handler);
				if (current >= 0) this.handlers.splice(current, 1);
			},
		};
	}

	dispatch(event: KeyboardEvent, context: TerminalKeyEventContext): boolean {
		for (const handler of this.handlers) {
			if (handler.when && !handler.when(context)) continue;
			if (handler.handle(event, context)) return true;
		}
		return false;
	}

	list(): readonly TerminalKeyEventHandler[] {
		return [...this.handlers];
	}

	clearScope(scope: 'surface' | 'application'): void {
		for (let index = this.handlers.length - 1; index >= 0; index--) {
			if ((this.handlers[index]?.scope ?? 'surface') === scope) this.handlers.splice(index, 1);
		}
	}

	dispose(): void {
		this.handlers.length = 0;
	}
}
