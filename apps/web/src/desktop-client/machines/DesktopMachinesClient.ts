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

const ColorSchema = z.enum(['cyan', 'grey', 'pink', 'purple']);

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
	card: z.strictObject({ lines: z.array(z.string()), notNow: z.string(), title: z.string() }),
	color: ColorSchema,
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

const LoginSchema = z.strictObject({
	asked: z.array(z.string()),
	ended: z.strictObject({ succeeded: z.boolean(), text: z.string() }).nullable(),
	kind: z.enum(['host-key', 'other', 'passphrase', 'password']),
	labels: z.strictObject({
		cancel: z.string(),
		close: z.string(),
		privacy: z.string().nullable(),
		rememberPrivacy: z.string(),
		sending: z.string(),
		submit: z.string(),
		title: z.string(),
		typeYes: z.string(),
	}),
	login: z.number().int().positive(),
	machine: z.string(),
	notice: z.string().nullable(),
	question: z.number().int().positive(),
	remember: z.strictObject({ label: z.string(), ticked: z.boolean() }).nullable(),
	rememberLine: z.string().nullable(),
	said: z.array(z.string()),
	sending: z.boolean(),
});

const AddSchema = z.strictObject({
	blocked: z.boolean(),
	color: ColorSchema,
	hosts: z.array(z.strictObject({ host: z.string(), label: z.string() })),
	lines: z.array(z.string()),
	problem: z
		.strictObject({ field: z.enum(['address', 'name']).nullable(), text: z.string() })
		.nullable(),
	running: z.boolean(),
	saved: z.string().nullable(),
	status: z.strictObject({ text: z.string(), tone: z.enum(['error', 'pending']) }).nullable(),
	submit: z.string(),
});

const OfferSchema = z.strictObject({
	accepted: z.boolean(),
	consequence: z.string().nullable(),
	count: z.string().nullable(),
	ended: z
		.strictObject({ text: z.string(), tone: z.enum(['error', 'success', 'warning']) })
		.nullable(),
	labels: z.strictObject({
		close: z.string(),
		decline: z.string(),
		logIn: z.string(),
		starting: z.string(),
		title: z.string(),
	}),
	login: z.number().int().positive(),
	machine: z.string(),
});

const ViewSchema = z.strictObject({
	add: z.strictObject({
		form: AddSchema.nullable(),
		labels: z.strictObject({
			add: z.string(),
			addMachine: z.string(),
			address: z.string(),
			cancel: z.string(),
			color: z.string(),
			hosts: z.string(),
			name: z.string(),
			nameHint: z.string(),
			title: z.string(),
		}),
	}),
	added: z.string().nullable(),
	catalog: z.enum(['read', 'unreadable']),
	change: z.strictObject({
		labels: z.strictObject({
			find: z.string(),
			heading: z.string(),
			here: z.string(),
			manage: z.string(),
			noMatch: z.string(),
			note: z.string(),
			title: z.string(),
		}),
		rows: z.array(
			z.strictObject({
				band: z.enum(['login', 'low', 'room', 'unusable']),
				machine: z.string().nullable(),
			}),
		),
	}),
	hub: z.string().nullable(),
	local: CapacitySchema,
	login: LoginSchema.nullable(),
	offer: OfferSchema.nullable(),
	machines: z.array(MachineSchema),
	move: z.strictObject({
		moved: z
			.strictObject({
				machine: z.string().nullable(),
				number: z.number().int().nonnegative(),
				workspace: z.string(),
			})
			.nullable(),
		notice: z.strictObject({ number: z.number().int().nonnegative(), text: z.string() }).nullable(),
		pending: z.string().nullable(),
		question: z
			.strictObject({
				cancel: z.string(),
				files: z.string(),
				move: z.string(),
				question: z.number().int().nonnegative(),
				sentence: z.string(),
				title: z.string(),
				workspace: z.string(),
			})
			.nullable(),
	}),
	notice: z.strictObject({ number: z.number().int().nonnegative(), text: z.string() }).nullable(),
	passwords: z.boolean(),
	seen: z.array(
		z.strictObject({
			machine: z.string(),
			projects: z.array(z.strictObject({ id: z.string(), name: z.string() })),
			workspaces: z.array(
				z.strictObject({ id: z.string(), name: z.string(), projectId: z.string().nullable() }),
			),
		}),
	),
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
