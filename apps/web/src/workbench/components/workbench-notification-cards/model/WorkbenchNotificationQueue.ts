interface WorkbenchNotificationRequest {
	/** Up to three button labels, in order; later ones are not shown. */
	readonly buttons?: readonly string[] | undefined;
	readonly message: string;
	readonly severity: 'error' | 'info' | 'warning';
}

export type WorkbenchNotificationOutcome =
	| { readonly index: number; readonly kind: 'chosen' }
	| { readonly kind: 'dismissed' };

/** A card as the region renders it. */
export interface WorkbenchNotificationCard {
	readonly buttons: readonly string[];
	readonly id: number;
	readonly message: string;
	readonly severity: WorkbenchNotificationRequest['severity'];
}

interface Entry {
	readonly card: WorkbenchNotificationCard;
	readonly settle: (outcome: WorkbenchNotificationOutcome) => void;
	timer: ReturnType<typeof setTimeout> | null;
}

const VISIBLE_LIMIT = 3;
const MAXIMUM_BUTTONS = 3;
/** How long a card without buttons that is not an error stays once it shows. */
const EXPIRING_CARD_MILLISECONDS = 10_000;
const DISMISSED = Object.freeze({ kind: 'dismissed' as const });
const NO_CARDS: readonly WorkbenchNotificationCard[] = Object.freeze([]);

/**
 * The Workbench's notification cards. The newest three show; an older card waits, unchanged, until
 * a newer one closes. A card without buttons that is not an error closes ten seconds after it last
 * began to show, and its clock stops while it waits; an error or a card with buttons stays until
 * the person answers it. Every card settles exactly once, with the chosen button or a dismissal.
 */
export class WorkbenchNotificationQueue {
	#entries: Entry[] = [];
	readonly #listeners = new Set<() => void>();
	#nextId = 1;
	#visible = NO_CARDS;

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	readonly getSnapshot = (): readonly WorkbenchNotificationCard[] => this.#visible;

	notify(request: WorkbenchNotificationRequest): Promise<WorkbenchNotificationOutcome> {
		return new Promise((settle) => {
			const id = this.#nextId;
			this.#nextId += 1;
			const card = Object.freeze({
				buttons: Object.freeze((request.buttons ?? []).slice(0, MAXIMUM_BUTTONS)),
				id,
				message: request.message,
				severity: request.severity,
			});
			this.#entries.push({ card, settle, timer: null });
			this.#update();
		});
	}

	choose(id: number, index: number): void {
		const entry = this.#entries.find((candidate) => candidate.card.id === id);
		if (entry !== undefined && index >= 0 && index < entry.card.buttons.length) {
			this.#remove(entry, { index, kind: 'chosen' });
		}
	}

	dismiss(id: number): void {
		const entry = this.#entries.find((candidate) => candidate.card.id === id);
		if (entry !== undefined) this.#remove(entry, DISMISSED);
	}

	/** Dismisses every card, as when the Workbench unmounts; later notifications still show. */
	close(): void {
		const entries = this.#entries;
		this.#entries = [];
		for (const entry of entries) {
			if (entry.timer !== null) clearTimeout(entry.timer);
			entry.settle(DISMISSED);
		}
		this.#update();
	}

	#remove(entry: Entry, outcome: WorkbenchNotificationOutcome): void {
		if (entry.timer !== null) clearTimeout(entry.timer);
		this.#entries = this.#entries.filter((candidate) => candidate !== entry);
		entry.settle(outcome);
		this.#update();
	}

	#update(): void {
		const firstVisible = Math.max(0, this.#entries.length - VISIBLE_LIMIT);
		this.#entries.forEach((entry, index) => {
			const expires = entry.card.severity !== 'error' && entry.card.buttons.length === 0;
			if (index < firstVisible || !expires) {
				if (entry.timer !== null) clearTimeout(entry.timer);
				entry.timer = null;
			} else if (entry.timer === null) {
				entry.timer = setTimeout(() => this.#remove(entry, DISMISSED), EXPIRING_CARD_MILLISECONDS);
			}
		});
		const visible = this.#entries.slice(firstVisible).map((entry) => entry.card);
		const unchanged =
			visible.length === this.#visible.length &&
			visible.every((card, index) => card === this.#visible[index]);
		if (unchanged) return;
		this.#visible = visible.length === 0 ? NO_CARDS : Object.freeze(visible);
		for (const listener of this.#listeners) listener();
	}
}
