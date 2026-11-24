import type { OrchbeeLocale } from '@orchbee/i18n';
import { z } from 'zod';

import type {
	TerminalSettingsMachineIntent,
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../../terminal-settings-surface';

const CapacitySchema = z.strictObject({
	long: z.string(),
	parts: z.array(z.strictObject({ low: z.boolean(), text: z.string() })).max(3),
	summary: z.string(),
});

const CountSchema = z.number().int().nonnegative().nullable();

const MachineSchema = z.strictObject({
	action: z
		.strictObject({
			act: z.enum(['connect', 'disconnect', 'log-in', 'retry', 'retry-now']),
			label: z.string(),
		})
		.nullable(),
	address: z.string().nullable(),
	agents: CountSchema,
	capacity: CapacitySchema,
	color: z.enum(['cyan', 'grey', 'pink', 'purple']),
	label: z.string(),
	lines: z.array(z.string()),
	machine: z.string(),
	notConnected: z.string(),
	removeDescription: z.string(),
	state: z.string(),
	status: z.string(),
	stopConfirmation: z.string(),
	stopping: z.boolean(),
	terminals: CountSchema,
	version: z.string().nullable(),
	word: z.string(),
	workspaces: CountSchema,
});

const ViewSchema = z.strictObject({
	added: z.string().nullable(),
	catalog: z.enum(['read', 'unreadable']),
	hub: z.string().nullable(),
	local: CapacitySchema,
	machines: z.array(MachineSchema),
	notice: z.strictObject({ number: z.number().int().nonnegative(), text: z.string() }).nullable(),
	passwords: z.boolean(),
});

const ProcessRecordSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('view'),
		sequence: z.number().int().nonnegative(),
		version: z.literal(1),
		view: ViewSchema,
	}),
	z.strictObject({
		kind: z.literal('outcome'),
		outcome: z.unknown(),
		request: z.number().int(),
		version: z.literal(1),
	}),
]);

/** The largest request number the process admits, 2⁵³ − 1. */
const LAST_REQUEST = Number.MAX_SAFE_INTEGER;

/**
 * This document's machines port: the machines process's views, which it acknowledges one by one,
 * and the requests, language and capacity reading Settings › Machines asks of it. Main carries the
 * text both ways without reading it.
 */
export class DesktopMachinesClient implements TerminalSettingsMachines {
	#capacityShown = 0;
	#disposed = false;
	#locale: OrchbeeLocale | null = null;
	#request = 0;
	#view: TerminalSettingsMachinesView | null = null;
	readonly #listeners = new Set<() => void>();
	readonly #port: MessagePort;

	constructor(port: MessagePort, locale: OrchbeeLocale) {
		this.#port = port;
		port.addEventListener('message', ({ data }) => this.#receive(data));
		port.start();
		this.setLocale(locale);
	}

	readonly getSnapshot = (): TerminalSettingsMachinesView | null => this.#view;

	readonly request = (intent: TerminalSettingsMachineIntent): void => {
		if (this.#request === LAST_REQUEST) return;
		this.#request += 1;
		this.#send({ intent, kind: 'intent', request: this.#request });
	};

	readonly showCapacity = (): (() => void) => {
		this.#capacityShown += 1;
		if (this.#capacityShown === 1) this.#send({ capacity: true, kind: 'watch' });
		let released = false;
		return () => {
			if (released) return;
			released = true;
			this.#capacityShown -= 1;
			if (this.#capacityShown === 0) this.#send({ capacity: false, kind: 'watch' });
		};
	};

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	/** The language the process words every machine fact in. */
	setLocale(locale: OrchbeeLocale): void {
		if (locale === this.#locale) return;
		this.#locale = locale;
		this.#send({ kind: 'locale', locale });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#listeners.clear();
		this.#port.close();
	}

	#receive(data: unknown): void {
		if (this.#disposed || typeof data !== 'string') return;
		let value: unknown;
		try {
			value = JSON.parse(data);
		} catch {
			return;
		}
		const record = ProcessRecordSchema.safeParse(value);
		// An outcome says only whether the request was well formed; the next view shows what came of it.
		if (!record.success || record.data.kind !== 'view') return;
		this.#view = record.data.view;
		// The next view waits for this acknowledgment, and the changes meanwhile coalesce into it.
		this.#send({ kind: 'ack', sequence: record.data.sequence });
		for (const listener of this.#listeners) listener();
	}

	#send(record: Readonly<Record<string, unknown>>): void {
		if (this.#disposed) return;
		this.#port.postMessage(JSON.stringify({ ...record, version: 1 }));
	}
}
