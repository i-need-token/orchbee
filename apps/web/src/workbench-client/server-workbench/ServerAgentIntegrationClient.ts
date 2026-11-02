import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationManagementFailureCode,
	type DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';
import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import type {
	AgentIntegrationManagementClientOperationResult,
	AgentIntegrationManagementClientSnapshot,
	AgentIntegrationManagementControl,
} from '../../agent-integration-management';
import {
	type ServerConnection,
	ServerStreamRefusedError,
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type JobAction = Extract<
	WorkbenchRequest,
	{ readonly kind: 'workbench.agent-integration' }
>['action'];
type JobOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.agent-integration.result' }
>['outcome'];
type Job = Extract<JobOutcome, { readonly kind: 'accepted' }>['job'];
type Report = Extract<JobOutcome, { readonly kind: 'completed' }>['report'];
type ReportedItem = Extract<Report, { readonly kind: 'list' }>['items'][number];
type MutationReport = Extract<Report, { readonly kind: 'mutation' }>['report'];
type Verdict = MutationReport['verdict'];
type Rows = AgentIntegrationManagementClientSnapshot['integrations'];
type FailureCode = DesktopAgentIntegrationManagementFailureCode;

interface Mutation {
	readonly direction: 'disable' | 'enable';
	readonly integrationId: DesktopAgentIntegrationId;
}

/** How a job ended for this window, as the CLI's job driver reports it. */
type JobEnd =
	| { readonly kind: 'completed'; readonly report: Report }
	| { readonly kind: 'deadline' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'exchange'; readonly started: boolean; readonly submitted: boolean }
	| {
			readonly kind: 'failed';
			readonly reason: Extract<JobOutcome, { readonly kind: 'failed' }>['reason'];
			readonly started: boolean;
	  };

type SettledEnd = Exclude<JobEnd, { readonly kind: 'disposed' }>;

type Exchanged =
	| { readonly kind: 'answered'; readonly outcome: JobOutcome }
	| { readonly kind: 'lost'; readonly submitted: boolean };

interface Settlement {
	readonly code: FailureCode | null;
	readonly integrations: Rows;
}

const EXCHANGE_DEADLINE_MILLISECONDS = 5_000;
/** The Worker ends an operation within 40 s; like the CLI, a job gets 50 s before it is cancelled. */
const JOB_DEADLINE_MILLISECONDS = 50_000;
const FIRST_POLL_MILLISECONDS = 20;
const POLL_CEILING_MILLISECONDS = 250;

/**
 * Agent integrations through the Session's Server, which runs each operation as a job on the
 * Worker, the node-local writer the CLI also uses. As the Desktop process did, rows stay uninspected
 * until Refresh, and the Worker alone decides whether a change may proceed. Agent launches are not
 * observed on this path yet, so no launch reminder is offered.
 */
export class ServerAgentIntegrationClient implements AgentIntegrationManagementControl {
	readonly #connection: Pick<ServerConnection, 'ended' | 'exchange'>;
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#disposed = false;
	#operating = false;
	#snapshot: AgentIntegrationManagementClientSnapshot = {
		failureCode: null,
		failureIntegrationId: null,
		integrations: DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: 'unknown',
			integrationId,
			issue: null,
			managementMode: 'managed-local',
			state: 'unknown',
		})),
		launchedIntegrationIds: [],
		pending: false,
		pendingIntegrationId: null,
		revision: 0,
	};

	constructor(connection: Pick<ServerConnection, 'ended' | 'exchange'>, sessionId: string) {
		this.#connection = connection;
		this.#sessionId = sessionId;
	}

	getSnapshot = (): AgentIntegrationManagementClientSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	refresh(): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate(null);
	}

	enable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({ direction: 'enable', integrationId });
	}

	disable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({ direction: 'disable', integrationId });
	}

	/** A job already started keeps running on the Worker; only this window stops waiting for it. */
	dispose(): void {
		this.#disposed = true;
		this.#listeners.clear();
	}

	async #operate(
		mutation: Mutation | null,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		if (this.#disposed) return { kind: 'unavailable' };
		const integrationId = mutation?.integrationId ?? null;
		if (this.#operating) {
			return { code: 'busy', integrationId, kind: 'failed', snapshot: this.#protocolSnapshot() };
		}
		this.#operating = true;
		this.#publish({
			failureCode: null,
			failureIntegrationId: null,
			pending: true,
			pendingIntegrationId: integrationId,
		});
		const end = await this.#runJob(
			mutation === null
				? { kind: 'list' }
				: { kind: mutation.direction, provider: mutation.integrationId },
		);
		this.#operating = false;
		if (this.#disposed || end.kind === 'disposed') return { kind: 'unavailable' };
		const { integrations } = this.#snapshot;
		const { code, integrations: settled } =
			mutation === null
				? settleRefresh(end, integrations)
				: settleMutation(end, mutation, integrations);
		this.#publish({
			failureCode: code,
			failureIntegrationId: code === null ? null : integrationId,
			integrations: settled,
			pending: false,
			pendingIntegrationId: null,
		});
		const snapshot = this.#protocolSnapshot();
		return code === null
			? { kind: 'completed', snapshot }
			: { code, integrationId, kind: 'failed', snapshot };
	}

	/** Starts one operation and polls it to its report; past the deadline it asks the job to stop. */
	async #runJob(
		operation: Extract<JobAction, { readonly kind: 'start' }>['request']['operation'],
	): Promise<JobEnd> {
		const deadline = performance.now() + JOB_DEADLINE_MILLISECONDS;
		const started = await this.#exchange({ kind: 'start', request: { operation } }, deadline);
		if (started.kind === 'lost') {
			return { kind: 'exchange', started: false, submitted: started.submitted };
		}
		if (started.outcome.kind === 'failed') {
			return { kind: 'failed', reason: started.outcome.reason, started: false };
		}
		if (started.outcome.kind !== 'accepted') {
			return { kind: 'failed', reason: 'malformed', started: false };
		}
		const { job } = started.outcome;
		for (
			let pause = FIRST_POLL_MILLISECONDS;
			;
			pause = Math.min(pause * 2, POLL_CEILING_MILLISECONDS)
		) {
			const remaining = deadline - performance.now();
			if (remaining <= 0) {
				void this.#exchange(
					{ job, kind: 'cancel' },
					performance.now() + EXCHANGE_DEADLINE_MILLISECONDS,
				);
				return { kind: 'deadline' };
			}
			await delay(Math.min(pause, remaining));
			if (this.#disposed) return { kind: 'disposed' };
			const polled = await this.#exchange({ job, kind: 'poll' }, deadline);
			if (polled.kind === 'lost') {
				return { kind: 'exchange', started: true, submitted: polled.submitted };
			}
			switch (polled.outcome.kind) {
				case 'pending':
					break;
				case 'completed':
					return { kind: 'completed', report: polled.outcome.report };
				case 'failed':
					return { kind: 'failed', reason: polled.outcome.reason, started: true };
				case 'accepted':
				case 'already-terminal':
				case 'cancel-registered':
					return { kind: 'failed', reason: 'malformed', started: true };
			}
		}
	}

	async #exchange(action: JobAction, deadline: number): Promise<Exchanged> {
		const requestId = createWorkbenchRequestId();
		const request = this.#encode(action, requestId);
		if (request === null || this.#connection.ended !== null)
			return { kind: 'lost', submitted: false };
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(request, {
				timeoutMilliseconds: Math.max(
					1,
					Math.min(EXCHANGE_DEADLINE_MILLISECONDS, deadline - performance.now()),
				),
			});
		} catch (error) {
			// Only a refused stream proves the Server never acted on the request.
			return { kind: 'lost', submitted: !(error instanceof ServerStreamRefusedError) };
		}
		try {
			const response = decodeWorkbenchResponse(record);
			if (
				response.kind === 'workbench.agent-integration.result' &&
				response.requestId === requestId &&
				response.sessionId === this.#sessionId &&
				answers(action, response.outcome)
			) {
				return { kind: 'answered', outcome: response.outcome };
			}
		} catch {
			// A result this window cannot read leaves the request's outcome unknown.
		}
		return { kind: 'lost', submitted: true };
	}

	#encode(action: JobAction, requestId: string): Uint8Array | null {
		try {
			const request = parseWorkbenchRequest({
				action,
				kind: 'workbench.agent-integration',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#sessionId,
			});
			return request.kind === 'workbench.agent-integration'
				? encodeWorkbenchRequest(request)
				: null;
		} catch {
			return null;
		}
	}

	#protocolSnapshot(): Extract<
		AgentIntegrationManagementClientOperationResult,
		{ readonly kind: 'completed' }
	>['snapshot'] {
		return {
			integrations: this.#snapshot.integrations,
			launchedIntegrationIds: [...this.#snapshot.launchedIntegrationIds],
			revision: this.#snapshot.revision,
		};
	}

	#publish(patch: Partial<Omit<AgentIntegrationManagementClientSnapshot, 'revision'>>): void {
		this.#snapshot = { ...this.#snapshot, ...patch, revision: this.#snapshot.revision + 1 };
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				// Isolate presentation observers from the operation they watch.
			}
		}
	}
}

function delay(milliseconds: number): Promise<void> {
	return new Promise((resolve) => {
		setTimeout(resolve, milliseconds);
	});
}

/** A result answers only its own action, and a poll or cancel only its own job. */
function answers(action: JobAction, outcome: JobOutcome): boolean {
	if (outcome.kind === 'failed') return true;
	switch (action.kind) {
		case 'start':
			return outcome.kind === 'accepted';
		case 'poll':
			return (
				(outcome.kind === 'pending' || outcome.kind === 'completed') &&
				sameJob(action.job, outcome.job)
			);
		case 'cancel':
			return (
				(outcome.kind === 'cancel-registered' || outcome.kind === 'already-terminal') &&
				sameJob(action.job, outcome.job)
			);
	}
}

function sameJob(left: Job, right: Job): boolean {
	return (
		left.workerId === right.workerId &&
		left.generation === right.generation &&
		left.jobId === right.jobId
	);
}

/** A read that did not finish leaves the rows as they were. */
function settleRefresh(end: SettledEnd, rows: Rows): Settlement {
	if (end.kind === 'completed' && end.report.kind === 'list') {
		return { code: null, integrations: listedRows(end.report.items, rows) };
	}
	let code: FailureCode = 'control-unavailable';
	// A Worker without a home directory, or one that ran out of time, read nothing.
	if (end.kind === 'deadline') code = 'operation-failed';
	else if (end.kind === 'completed' && end.report.kind === 'home-unavailable') {
		code = 'operation-failed';
	} else if (end.kind === 'failed' && end.reason === 'busy' && !end.started) code = 'busy';
	return { code, integrations: rows };
}

/** A change that may have begun is never reported as refused; its row waits for a Refresh. */
function settleMutation(end: SettledEnd, mutation: Mutation, rows: Rows): Settlement {
	const uncertain: Settlement = {
		code: 'verification-failed',
		integrations: uncertainRows(rows, mutation.integrationId),
	};
	switch (end.kind) {
		case 'completed': {
			const { report } = end;
			if (report.kind === 'home-unavailable')
				return { code: 'operation-failed', integrations: rows };
			if (
				report.kind !== 'mutation' ||
				report.report.integrationId !== mutation.integrationId ||
				report.report.direction !== mutation.direction
			) {
				return uncertain;
			}
			return {
				code: verdictFailure(report.report.verdict, report.report.effectBegun),
				integrations: mutatedRows(rows, report.report, mutation.integrationId),
			};
		}
		case 'deadline':
			return uncertain;
		case 'failed':
			if (end.started || end.reason === 'abandoned') return uncertain;
			return { code: end.reason === 'busy' ? 'busy' : 'control-unavailable', integrations: rows };
		case 'exchange':
			return end.started || end.submitted
				? uncertain
				: { code: 'control-unavailable', integrations: rows };
	}
}

function verdictFailure(verdict: Verdict, effectBegun: boolean): FailureCode | null {
	switch (verdict.kind) {
		case 'completed':
			return null;
		case 'refused':
			return refusalFailure(verdict.code, effectBegun);
		case 'failed':
			return postEffectFailure(verdict.code);
		case 'cancelled':
		case 'unknown':
			return 'verification-failed';
	}
}

function refusalFailure(
	code: Extract<Verdict, { readonly kind: 'refused' }>['code'],
	effectBegun: boolean,
): FailureCode {
	switch (code) {
		case 'agent-unavailable':
		case 'conflict':
		case 'manual-action-required':
			return code;
		case 'capsule-unavailable':
		case 'inspection-failed':
			return 'operation-failed';
		case 'timed-out':
			return effectBegun ? 'verification-failed' : 'operation-failed';
	}
}

function postEffectFailure(
	code: Extract<Verdict, { readonly kind: 'failed' }>['code'],
): FailureCode {
	switch (code) {
		case 'conflict':
		case 'manual-action-required':
		case 'operation-failed':
			return code;
		case 'leaked':
		case 'output-too-large':
		case 'timed-out':
		case 'verification-failed':
			return 'verification-failed';
	}
}

function reportedRow(
	item: ReportedItem,
	integrationId: DesktopAgentIntegrationId,
): DesktopAgentIntegrationSnapshotItem {
	return {
		availability: item.availability,
		integrationId,
		issue: item.issue ?? null,
		managementMode: item.managementMode,
		state: item.state,
	};
}

function listedRows(items: readonly ReportedItem[], rows: Rows): Rows {
	return rows.map((row) => {
		const item = items.find((candidate) => candidate.integrationId === row.integrationId);
		return item === undefined
			? { ...row, availability: 'unknown', issue: 'inspection-failed', state: 'unknown' }
			: reportedRow(item, row.integrationId);
	});
}

function uncertainRows(rows: Rows, integrationId: DesktopAgentIntegrationId): Rows {
	return rows.map((row) =>
		row.integrationId === integrationId ? { ...row, issue: null, state: 'unknown' } : row,
	);
}

function mutatedRows(
	rows: Rows,
	report: MutationReport,
	integrationId: DesktopAgentIntegrationId,
): Rows {
	const { observed } = report;
	if (observed?.integrationId === integrationId) {
		return rows.map((row) =>
			row.integrationId === integrationId ? reportedRow(observed, integrationId) : row,
		);
	}
	return report.effectBegun ? uncertainRows(rows, integrationId) : rows;
}
