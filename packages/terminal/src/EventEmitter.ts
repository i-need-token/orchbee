/**
 * Typed event emitter (xterm.js-compatible pattern).
 *
 * Listeners are invoked synchronously. Subscribing returns an IDisposable
 * for easy unsubscription.
 */

export interface IDisposable {
	dispose(): void;
}

export type IEvent<T> = (listener: (arg: T) => void) => IDisposable;

export class EventEmitter<T> {
	private listeners: Array<(arg: T) => void> = [];

	fire(arg: T): void {
		for (const listener of this.listeners.slice()) {
			listener(arg);
		}
	}

	event: IEvent<T> = (listener) => {
		this.listeners.push(listener);
		return {
			dispose: () => {
				const index = this.listeners.indexOf(listener);
				if (index >= 0) {
					this.listeners.splice(index, 1);
				}
			},
		};
	};

	dispose(): void {
		this.listeners = [];
	}
}
