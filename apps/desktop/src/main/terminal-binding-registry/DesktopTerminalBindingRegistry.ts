import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

import type { TerminalBindingId } from '../recovery';

export interface DesktopTerminalBindingRegistration {
	readonly bindingId: TerminalBindingId;
	readonly presentationId: TerminalPresentationId;
}

export type DesktopTerminalBindingRegistrationResult =
	| {
			readonly kind: 'registered' | 'existing';
			readonly registration: DesktopTerminalBindingRegistration;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: 'binding-id-conflict' | 'presentation-id-conflict';
	  };

export type DesktopTerminalBindingRemovalResult =
	| { readonly kind: 'removed'; readonly registration: DesktopTerminalBindingRegistration }
	| { readonly kind: 'unchanged' }
	| { readonly kind: 'rejected'; readonly reason: 'stale-binding' };

export class DesktopTerminalBindingRegistry {
	readonly #byBindingId = new Map<TerminalBindingId, DesktopTerminalBindingRegistration>();
	readonly #byPresentationId = new Map<
		TerminalPresentationId,
		DesktopTerminalBindingRegistration
	>();

	get size(): number {
		return this.#byPresentationId.size;
	}

	getByBindingId(bindingId: TerminalBindingId): DesktopTerminalBindingRegistration | null {
		return this.#byBindingId.get(bindingId) ?? null;
	}

	getByPresentationId(
		presentationId: TerminalPresentationId,
	): DesktopTerminalBindingRegistration | null {
		return this.#byPresentationId.get(presentationId) ?? null;
	}

	getSnapshot(): readonly DesktopTerminalBindingRegistration[] {
		return [...this.#byPresentationId.values()].sort((left, right) =>
			comparePresentationIds(left.presentationId, right.presentationId),
		);
	}

	register(
		registration: DesktopTerminalBindingRegistration,
	): DesktopTerminalBindingRegistrationResult {
		const presentationRegistration = this.#byPresentationId.get(registration.presentationId);
		if (presentationRegistration !== undefined) {
			return presentationRegistration.bindingId === registration.bindingId
				? { kind: 'existing', registration: presentationRegistration }
				: { kind: 'rejected', reason: 'presentation-id-conflict' };
		}
		const bindingRegistration = this.#byBindingId.get(registration.bindingId);
		if (bindingRegistration !== undefined) {
			return { kind: 'rejected', reason: 'binding-id-conflict' };
		}
		const canonical = { ...registration };
		this.#byBindingId.set(canonical.bindingId, canonical);
		this.#byPresentationId.set(canonical.presentationId, canonical);
		this.#assertIntegrity();
		return { kind: 'registered', registration: canonical };
	}

	remove(
		presentationId: TerminalPresentationId,
		expectedBindingId: TerminalBindingId,
	): DesktopTerminalBindingRemovalResult {
		const registration = this.#byPresentationId.get(presentationId);
		if (registration === undefined) return { kind: 'unchanged' };
		if (registration.bindingId !== expectedBindingId) {
			return { kind: 'rejected', reason: 'stale-binding' };
		}
		this.#byPresentationId.delete(presentationId);
		this.#byBindingId.delete(expectedBindingId);
		this.#assertIntegrity();
		return { kind: 'removed', registration };
	}

	#assertIntegrity(): void {
		if (this.#byBindingId.size !== this.#byPresentationId.size) {
			throw new Error('Desktop terminal binding registry indexes diverged.');
		}
		for (const registration of this.#byPresentationId.values()) {
			if (this.#byBindingId.get(registration.bindingId) !== registration) {
				throw new Error('Desktop terminal binding registry lost its reverse index.');
			}
		}
	}
}

function comparePresentationIds(
	left: TerminalPresentationId,
	right: TerminalPresentationId,
): number {
	if (left < right) return -1;
	return left === right ? 0 : 1;
}
