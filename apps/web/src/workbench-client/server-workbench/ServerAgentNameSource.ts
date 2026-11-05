import type {
	WorkbenchAgentNames,
	WorkbenchAgentNameSource,
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
} from '../../workbench';
import {
	WORKBENCH_PROTOCOL_VERSION,
	type WorkbenchRequest,
	type WorkbenchResponse,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import {
	createClientServerRequestId,
	decodeAgentReadResult,
	decodeWorkbenchResponse,
	encodeAgentReadRequest,
	encodeWorkbenchRequest,
	type ServerConnection,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';
import { readServerTerminalTask } from './server-terminal-names';
import type { ServerTerminalAgentBinding } from './ServerTerminalAgentSource';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type PanelAgentOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.panel-agent.result' }
>['outcome'];
type PanelMetadataOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.panel-metadata.result' }
>['outcome'];
type ReportPanel = Extract<
	PanelAgentOutcome,
	{ readonly kind: 'presentations-read' }
>['presentations'][number]['panel'];
type MetadataPanel = Extract<
	PanelMetadataOutcome,
	{ readonly kind: 'presentations-read' }
>['presentations'][number]['panel'];

/** The TUI reads its Agent rows' names about this often. */
const ROUND_INTERVAL_MILLISECONDS = 2000;
/** One round's reads share this deadline; what a late round misses keeps what the last one read. */
const ROUND_DEADLINE_MILLISECONDS = 5000;
/** The most terminals the Server answers in one title, report or metadata read. */
const READ_ROWS = 4;

const NO_NAMES: ReadonlyMap<string, WorkbenchAgentNames> = new Map();

/** The Agents to name: the Server's observations and the terminal incarnation each was made in. */
interface ServerAgentNameTargets {
	getSnapshot(): WorkbenchAgentSnapshot;
	subscribe(listener: () => void): () => void;
	terminalBinding(observation: WorkbenchAgentObservation): ServerTerminalAgentBinding | undefined;
}

interface Target {
	readonly binding: ServerTerminalAgentBinding;
	readonly observation: WorkbenchAgentObservation;
}

/** What one round read for a terminal; a field left out was not read and keeps the last reading. */
interface Reading {
	name?: string | null;
	reportedTitle?: string | null;
	terminalTitle?: string | null;
}

/**
 * The names the TUI's Agent rows read from the Server, read the same way for this window: the name a
 * person gave an Agent, the title Agent reports and Panel metadata give its terminal, and the title
 * the terminal's program set, which this window otherwise sees only for terminals it shows. A round
 * reads every observed Agent within one deadline, four terminals a request as the Server answers,
 * and the next round follows two seconds after it, or at once when the observed Agents change.
 */
export class ServerAgentNameSource implements WorkbenchAgentNameSource {
	readonly #agents: ServerAgentNameTargets;
	readonly #connection: Pick<ServerConnection, 'exchange'>;
	readonly #listeners = new Set<() => void>();
	readonly #session: { readonly snapshot: Pick<WorkbenchSnapshot, 'sessionId'> };
	#again = false;
	#ended = false;
	#inFlight = false;
	#snapshot = NO_NAMES;
	#stopAgents: (() => void) | null = null;
	#stopEnd: (() => void) | null = null;
	#targetKey = '';
	#timer: ReturnType<typeof setTimeout> | null = null;

	constructor(
		connection: Pick<ServerConnection, 'exchange'>,
		session: {
			readonly onEnded: ServerWorkbenchSession['onEnded'];
			readonly snapshot: Pick<WorkbenchSnapshot, 'sessionId'>;
		},
		agents: ServerAgentNameTargets,
	) {
		this.#agents = agents;
		this.#connection = connection;
		this.#session = session;
		this.#stopEnd = session.onEnded(() => this.dispose());
		if (this.#ended) return;
		this.#stopAgents = agents.subscribe(() => this.#follow());
		this.#follow();
	}

	getSnapshot(): ReadonlyMap<string, WorkbenchAgentNames> {
		return this.#snapshot;
	}

	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		if (this.#ended) return;
		this.#ended = true;
		this.#stopEnd?.();
		this.#stopAgents?.();
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = null;
		this.#publish(NO_NAMES);
	}

	#targets(): Map<string, Target> {
		const targets = new Map<string, Target>();
		for (const [presentationId, observation] of this.#agents.getSnapshot().presentations) {
			const binding = this.#agents.terminalBinding(observation);
			if (binding !== undefined) targets.set(presentationId, { binding, observation });
		}
		return targets;
	}

	#follow(): void {
		if (this.#ended) return;
		// The Server's observations keep their order, so the same Agents give the same key.
		const key = [...this.#targets()]
			.map(([presentationId, { observation }]) => `${presentationId}:${observation.binding}`)
			.join('\0');
		if (key === this.#targetKey) return;
		this.#targetKey = key;
		if (this.#inFlight) {
			this.#again = true;
			return;
		}
		this.#start();
	}

	#start(): void {
		if (this.#timer !== null) clearTimeout(this.#timer);
		this.#timer = null;
		const targets = this.#targets();
		if (targets.size === 0) {
			this.#publish(NO_NAMES);
			return;
		}
		this.#inFlight = true;
		void this.#round(targets).then(() => {
			this.#inFlight = false;
			if (this.#ended) return;
			if (this.#again) {
				this.#again = false;
				this.#start();
				return;
			}
			this.#timer = setTimeout(() => {
				this.#timer = null;
				this.#start();
			}, ROUND_INTERVAL_MILLISECONDS);
		});
	}

	async #round(targets: ReadonlyMap<string, Target>): Promise<void> {
		const deadline = performance.now() + ROUND_DEADLINE_MILLISECONDS;
		const readings = new Map<string, Reading>([...targets.keys()].map((id) => [id, {}]));
		const names = await this.#readNames(deadline);
		for (const [presentationId, { binding }] of targets) {
			const reading = readings.get(presentationId);
			if (names !== null && reading !== undefined) {
				reading.name = names.get(bindingKey(binding)) ?? null;
			}
		}
		const entries = [...targets];
		const chunks: (readonly (readonly [string, Target])[])[] = [];
		for (let start = 0; start < entries.length; start += READ_ROWS) {
			chunks.push(entries.slice(start, start + READ_ROWS));
		}
		// One chunk after another keeps what a round asks of the Server bounded.
		await chunks.reduce<Promise<void>>(
			(previous, chunk) => previous.then(() => this.#readChunk(chunk, readings, deadline)),
			Promise.resolve(),
		);
		if (!this.#ended) this.#merge(targets, readings);
	}

	async #readChunk(
		chunk: readonly (readonly [string, Target])[],
		readings: ReadonlyMap<string, Reading>,
		deadline: number,
	): Promise<void> {
		const presentationIds = chunk.map(([, { binding }]) => binding.presentationId);
		const [titles, reports, metadata] = await Promise.all([
			this.#exchange(
				{ bindings: chunk.map(([, { binding }]) => binding), kind: 'titles' },
				deadline,
			),
			this.#exchange({ kind: 'read-presentations', presentationIds }, deadline),
			this.#exchangeMetadata(presentationIds, deadline),
		]);
		for (const [presentationId, { binding, observation }] of chunk) {
			const reading = readings.get(presentationId);
			if (reading === undefined) continue;
			if (titles?.kind === 'titles') {
				const row = titles.presentations.find(
					(candidate) => bindingKey(candidate.binding) === bindingKey(binding),
				);
				if (row !== undefined) {
					reading.terminalTitle =
						row.observation.kind === 'observed' && row.observation.title !== null
							? readServerTerminalTask(row.observation.title)
							: null;
				}
			}
			const report =
				reports?.kind === 'presentations-read'
					? reports.presentations.find((row) => row.presentationId === presentationId)?.panel
					: undefined;
			const panel = metadata?.presentations.find(
				(row) => row.presentationId === presentationId,
			)?.panel;
			if (report !== undefined && panel !== undefined && samePanelTerminal(report, panel)) {
				reading.reportedTitle = effectiveTitle(panel, observation.agent, report);
			}
		}
	}

	/** The names people gave Agents' terminals, by exact incarnation; null when the read failed. */
	async #readNames(deadline: number): Promise<ReadonlyMap<string, string> | null> {
		const remaining = Math.floor(deadline - performance.now());
		if (remaining <= 0) return null;
		const requestId = createClientServerRequestId();
		try {
			const result = decodeAgentReadResult(
				await this.#connection.exchange(
					encodeAgentReadRequest({ kind: 'agent.session.names.list.request', requestId }),
					{ timeoutMilliseconds: remaining },
				),
			);
			if (result.kind !== 'agent.session.names.list.result' || result.requestId !== requestId) {
				return null;
			}
			const names = new Map<string, string>();
			const { outcome } = result.result;
			// As in the TUI, only terminal targets name a row, and a launch's chosen name names its
			// terminal while it starts; a Server that lists sessions only names none.
			if (outcome.kind !== 'listed-targets') return names;
			for (const { name, target } of outcome.bindings) {
				if (target.kind === 'terminal') names.set(bindingKey(target.binding), name);
			}
			for (const { binding, name } of outcome.pending) names.set(bindingKey(binding), name);
			return names;
		} catch {
			return null;
		}
	}

	async #exchange(
		action: Extract<WorkbenchRequest, { readonly kind: 'workbench.panel-agent' }>['action'],
		deadline: number,
	): Promise<PanelAgentOutcome | null> {
		const requestId = createWorkbenchRequestId();
		const response = await this.#request(
			{
				action,
				kind: 'workbench.panel-agent',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#session.snapshot.sessionId,
			},
			deadline,
		);
		return response?.kind === 'workbench.panel-agent.result' && response.requestId === requestId
			? response.outcome
			: null;
	}

	async #exchangeMetadata(
		presentationIds: readonly ServerTerminalAgentBinding['presentationId'][],
		deadline: number,
	): Promise<Extract<PanelMetadataOutcome, { readonly kind: 'presentations-read' }> | null> {
		const requestId = createWorkbenchRequestId();
		const response = await this.#request(
			{
				action: { kind: 'read-presentations', presentationIds },
				kind: 'workbench.panel-metadata',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#session.snapshot.sessionId,
			},
			deadline,
		);
		return response?.kind === 'workbench.panel-metadata.result' &&
			response.requestId === requestId &&
			response.outcome.kind === 'presentations-read'
			? response.outcome
			: null;
	}

	/** One exchange within what remains of the round; null when no answer came back in time. */
	async #request(request: WorkbenchRequest, deadline: number): Promise<WorkbenchResponse | null> {
		const remaining = Math.floor(deadline - performance.now());
		if (remaining <= 0) return null;
		try {
			return decodeWorkbenchResponse(
				await this.#connection.exchange(encodeWorkbenchRequest(request), {
					timeoutMilliseconds: remaining,
				}),
			);
		} catch {
			return null;
		}
	}

	/** Keeps each unchanged entry's identity, so a round that changes nothing publishes nothing. */
	#merge(targets: ReadonlyMap<string, Target>, readings: ReadonlyMap<string, Reading>): void {
		const next = new Map<string, WorkbenchAgentNames>();
		for (const [presentationId, { observation }] of targets) {
			const reading = readings.get(presentationId) ?? {};
			const prior = this.#snapshot.get(presentationId);
			const kept = prior?.binding === observation.binding ? prior : undefined;
			if (kept === undefined && Object.keys(reading).length === 0) continue;
			const names: WorkbenchAgentNames = {
				binding: observation.binding,
				name: reading.name === undefined ? (kept?.name ?? null) : reading.name,
				reportedTitle:
					reading.reportedTitle === undefined
						? (kept?.reportedTitle ?? null)
						: reading.reportedTitle,
				terminalTitle:
					reading.terminalTitle === undefined ? kept?.terminalTitle : reading.terminalTitle,
			};
			next.set(
				presentationId,
				kept?.name === names.name &&
					kept.reportedTitle === names.reportedTitle &&
					kept.terminalTitle === names.terminalTitle
					? kept
					: Object.freeze(names),
			);
		}
		const current = this.#snapshot;
		if (
			next.size !== current.size ||
			[...next].some(([presentationId, names]) => current.get(presentationId) !== names)
		) {
			this.#publish(next);
		}
	}

	#publish(snapshot: ReadonlyMap<string, WorkbenchAgentNames>): void {
		if (snapshot === this.#snapshot || (snapshot.size === 0 && this.#snapshot.size === 0)) return;
		this.#snapshot = snapshot;
		for (const listener of this.#listeners) listener();
	}
}

function bindingKey({
	generation,
	hostEpoch,
	hostTerminalId,
	presentationId,
}: Readonly<
	Record<'generation' | 'hostEpoch' | 'hostTerminalId' | 'presentationId', string>
>): string {
	return `${presentationId}:${generation}:${hostEpoch}:${hostTerminalId}`;
}

/** Both reads saw the Presentation in the same Panel on the same terminal. */
function samePanelTerminal(report: ReportPanel, metadata: MetadataPanel): boolean {
	return (
		report.panelId === metadata.panelId &&
		report.binding !== null &&
		report.binding.terminalId === metadata.binding?.terminalId &&
		report.binding.workerId === metadata.binding.workerId
	);
}

/**
 * The Panel's effective title as the Server's own rule chooses it: among the records whose Agent and
 * report-source guards the Agent satisfies, the title written last. A guard the Agent's facts leave
 * unknown is never satisfied.
 */
function effectiveTitle(
	metadata: MetadataPanel,
	agent: string,
	report: ReportPanel,
): string | null {
	const authoritySource = report.report?.agent === agent ? report.report.authoritySource : null;
	let effective: { readonly order: bigint; readonly value: string } | null = null;
	for (const record of metadata.presentations) {
		if (
			record.title === null ||
			(record.agent !== null && record.agent !== agent) ||
			(record.appliesToSource !== null && record.appliesToSource !== authoritySource)
		) {
			continue;
		}
		const order = BigInt(record.title.order);
		if (effective === null || order > effective.order) {
			effective = { order, value: record.title.value };
		}
	}
	return effective?.value ?? null;
}
