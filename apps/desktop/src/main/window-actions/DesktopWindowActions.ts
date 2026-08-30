import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
	type ActionInvocationSource,
	type ActionSignature,
} from '@orchbee/actions';
import { desktopWindowMessages } from '@orchbee/i18n';

const DESKTOP_WINDOW_ACTION_IDS = {
	toggleFullScreen: 'desktop.window.toggle-full-screen',
	toggleMaximized: 'desktop.window.toggle-maximized',
} as const;

interface DesktopWindowActionCatalog {
	readonly [DESKTOP_WINDOW_ACTION_IDS.toggleFullScreen]: ActionSignature<void>;
	readonly [DESKTOP_WINDOW_ACTION_IDS.toggleMaximized]: ActionSignature<void>;
}

export interface DesktopWindowActionTarget {
	getWindowState(): 'full-screen' | 'maximized' | 'normal' | 'unavailable';
	toggleWindowFullScreen(): boolean;
	toggleWindowMaximized(): boolean;
}

export type DesktopWindowActionSource = Extract<ActionInvocationSource, 'keyboard' | 'menu'>;

export class DesktopWindowActions {
	readonly #registrations: readonly ActionDisposable[];
	readonly #runtime = new ActionRuntime();
	readonly #scope = this.#runtime.createScope('desktop.window');
	readonly #target: DesktopWindowActionTarget;
	readonly #typedRuntime = createTypedActionRuntime<DesktopWindowActionCatalog>(this.#runtime);

	constructor(target: DesktopWindowActionTarget) {
		this.#target = target;
		const category = { id: 'window', title: desktopWindowMessages.category };
		const execution = {
			concurrency: 'parallel' as const,
			concurrencyKey: () => 'desktop-window',
			mode: 'foreground' as const,
		};
		this.#registrations = [
			this.#typedRuntime.register(
				this.#scope,
				defineAction({
					category,
					execution,
					getAvailability: () => this.#fullScreenAvailability(),
					handler: {
						execute: (context) => {
							if (!this.#target.toggleWindowFullScreen()) return { kind: 'unchanged' };
							context.markCommitted();
							return { kind: 'completed' };
						},
					},
					id: DESKTOP_WINDOW_ACTION_IDS.toggleFullScreen,
					input: noActionInput,
					title: desktopWindowMessages.toggleFullScreen,
				}),
			),
			this.#typedRuntime.register(
				this.#scope,
				defineAction({
					category,
					execution,
					getAvailability: () => this.#maximizedAvailability(),
					handler: {
						execute: (context) => {
							if (!this.#target.toggleWindowMaximized()) return { kind: 'unchanged' };
							context.markCommitted();
							return { kind: 'completed' };
						},
					},
					id: DESKTOP_WINDOW_ACTION_IDS.toggleMaximized,
					input: noActionInput,
					title: desktopWindowMessages.toggleMaximized,
				}),
			),
		];
	}

	dispose(): void {
		for (const registration of this.#registrations) registration.dispose();
		this.#runtime.dispose();
	}

	getToggleFullScreenAvailability(): ActionAvailability {
		return this.#fullScreenAvailability();
	}

	getToggleMaximizedAvailability(): ActionAvailability {
		return this.#maximizedAvailability();
	}

	invokeToggleFullScreen(source: DesktopWindowActionSource): boolean {
		return this.#invoke(DESKTOP_WINDOW_ACTION_IDS.toggleFullScreen, source);
	}

	invokeToggleMaximized(source: DesktopWindowActionSource): boolean {
		return this.#invoke(DESKTOP_WINDOW_ACTION_IDS.toggleMaximized, source);
	}

	#fullScreenAvailability(): ActionAvailability {
		const state = this.#target.getWindowState();
		return state === 'unavailable'
			? { kind: 'disabled', reason: desktopWindowMessages.unavailable }
			: { checked: state === 'full-screen', kind: 'enabled' };
	}

	#invoke(actionId: keyof DesktopWindowActionCatalog, source: DesktopWindowActionSource): boolean {
		const invocation = { scope: this.#scope, source };
		const resolved = this.#typedRuntime.resolveAction(actionId, undefined, invocation);
		if (resolved?.availability.kind !== 'enabled') return false;
		void this.#typedRuntime.execute(actionId, undefined, invocation).completion;
		return true;
	}

	#maximizedAvailability(): ActionAvailability {
		const state = this.#target.getWindowState();
		if (state === 'unavailable') {
			return { kind: 'disabled', reason: desktopWindowMessages.unavailable };
		}
		if (state === 'full-screen') {
			return {
				kind: 'disabled',
				reason: desktopWindowMessages.maximizedUnavailableInFullScreen,
			};
		}
		return { checked: state === 'maximized', kind: 'enabled' };
	}
}
