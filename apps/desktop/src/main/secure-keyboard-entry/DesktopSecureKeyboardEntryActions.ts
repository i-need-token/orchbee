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
import { desktopSecureKeyboardEntryMessages } from '@orchbee/i18n';

import type { DesktopSecureKeyboardEntry } from './DesktopSecureKeyboardEntry';

const TOGGLE_SECURE_KEYBOARD_ENTRY_ACTION_ID = 'desktop.secure-keyboard-entry.toggle';

interface DesktopSecureKeyboardEntryActionCatalog {
	readonly [TOGGLE_SECURE_KEYBOARD_ENTRY_ACTION_ID]: ActionSignature<void>;
}

export type DesktopSecureKeyboardEntryActionSource = Extract<ActionInvocationSource, 'menu'>;

export class DesktopSecureKeyboardEntryActions {
	readonly #registrations: readonly ActionDisposable[];
	readonly #runtime = new ActionRuntime();
	readonly #scope = this.#runtime.createScope('desktop.secure-keyboard-entry');
	readonly #target: DesktopSecureKeyboardEntry;
	readonly #typedRuntime = createTypedActionRuntime<DesktopSecureKeyboardEntryActionCatalog>(
		this.#runtime,
	);

	constructor(target: DesktopSecureKeyboardEntry) {
		this.#target = target;
		this.#registrations = [
			this.#typedRuntime.register(
				this.#scope,
				defineAction({
					category: {
						id: 'input-security',
						title: desktopSecureKeyboardEntryMessages.category,
					},
					execution: {
						concurrency: 'serialize',
						concurrencyKey: () => 'desktop-secure-keyboard-entry',
						mode: 'foreground',
					},
					getAvailability: () => this.#availability(),
					handler: {
						execute: (context) => {
							if (!this.#target.toggle()) return { kind: 'unchanged' };
							context.markCommitted();
							return { kind: 'completed' };
						},
					},
					id: TOGGLE_SECURE_KEYBOARD_ENTRY_ACTION_ID,
					input: noActionInput,
					title: desktopSecureKeyboardEntryMessages.toggle,
				}),
			),
		];
	}

	dispose(): void {
		for (const registration of this.#registrations) registration.dispose();
		this.#runtime.dispose();
	}

	getToggleAvailability(): ActionAvailability {
		return this.#availability();
	}

	invokeToggle(source: DesktopSecureKeyboardEntryActionSource): boolean {
		const invocation = { scope: this.#scope, source };
		const resolved = this.#typedRuntime.resolveAction(
			TOGGLE_SECURE_KEYBOARD_ENTRY_ACTION_ID,
			undefined,
			invocation,
		);
		if (resolved?.availability.kind !== 'enabled') return false;
		void this.#typedRuntime.execute(TOGGLE_SECURE_KEYBOARD_ENTRY_ACTION_ID, undefined, invocation)
			.completion;
		return true;
	}

	#availability(): ActionAvailability {
		const state = this.#target.getState();
		if (state === 'unsupported') return { kind: 'hidden' };
		if (state === 'unavailable') {
			return { kind: 'disabled', reason: desktopSecureKeyboardEntryMessages.unavailable };
		}
		return { checked: state === 'active', kind: 'enabled' };
	}
}
