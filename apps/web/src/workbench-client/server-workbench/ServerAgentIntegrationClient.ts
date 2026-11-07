import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import {
	AGENT_INTEGRATION_IDS,
	type AgentIntegrationDiagnosis,
	type AgentIntegrationExplanation,
	type AgentIntegrationId,
	type AgentIntegrationManagementClientOperationResult,
	type AgentIntegrationManagementClientSnapshot,
	type AgentIntegrationManagementControl,
	type AgentIntegrationManagementFailureCode,
	type AgentIntegrationManagementSnapshot,
	type AgentIntegrationSnapshotItem,
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
type JobOperation = Extract<JobAction, { readonly kind: 'start' }>['request']['operation'];
type JobOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.agent-integration.result' }
>['outcome'];
type Job = Extract<JobOutcome, { readonly kind: 'accepted' }>['job'];
type Report = Extract<JobOutcome, { readonly kind: 'completed' }>['report'];
type ReportedItem = Extract<Report, { readonly kind: 'list' }>['items'][number];
type ExplainReport = Extract<Report, { readonly kind: 'explain' }>;
type MutationReport = Extract<Report, { readonly kind: 'mutation' }>['report'];
type Verdict = MutationReport['verdict'];
type Rows = AgentIntegrationManagementClientSnapshot['integrations'];
type FailureCode = AgentIntegrationManagementFailureCode;

interface Mutation {
	readonly direction: 'disable' | 'enable';
	readonly integrationId: AgentIntegrationId;
}

/** One explicit action; each runs as exactly one Worker job. */
type Operation =
	| { readonly kind: 'change'; readonly mutation: Mutation }
	| { readonly integrationId: AgentIntegrationId; readonly kind: 'explain' }
	| { readonly kind: 'refresh' };

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
	/** Present only for an explain job, whose failure belongs to its diagnosis. */
	readonly diagnosis?: AgentIntegrationDiagnosis;
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
 * until Refresh, and the Worker alone decides whether a change may proceed.
 */
export class ServerAgentIntegrationClient implements AgentIntegrationManagementControl {
	readonly #connection: Pick<ServerConnection, 'ended' | 'exchange'>;
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#disposed = false;
	#operating = false;
	#snapshot: AgentIntegrationManagementClientSnapshot = {
		diagnosis: null,
		failureCode: null,
		failureIntegrationId: null,
		integrations: AGENT_INTEGRATION_IDS.map((integrationId) => ({
			availability: 'unknown',
			integrationId,
			issue: null,
			managementMode: 'managed-local',
			state: 'unknown',
		})),
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
		return this.#operate({ kind: 'refresh' });
	}

	explain(
		integrationId: AgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({ integrationId, kind: 'explain' });
	}

	enable(
		integrationId: AgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({ kind: 'change', mutation: { direction: 'enable', integrationId } });
	}

	disable(
		integrationId: AgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult> {
		return this.#operate({ kind: 'change', mutation: { direction: 'disable', integrationId } });
	}

	/** A job already started keeps running on the Worker; only this window stops waiting for it. */
	dispose(): void {
		this.#disposed = true;
		this.#listeners.clear();
	}

	async #operate(operation: Operation): Promise<AgentIntegrationManagementClientOperationResult> {
		if (this.#disposed) return { kind: 'unavailable' };
		const integrationId = operationTarget(operation);
		if (this.#operating) {
			return { code: 'busy', integrationId, kind: 'failed', snapshot: this.#protocolSnapshot() };
		}
		this.#operating = true;
		this.#publish({
			...(operation.kind === 'explain'
				? { diagnosis: { integrationId: operation.integrationId, kind: 'pending' } }
				: {}),
			failureCode: null,
			failureIntegrationId: null,
			pending: true,
			pendingIntegrationId: integrationId,
		});
		const end = await this.#runJob(jobOperation(operation));
		this.#operating = false;
		if (this.#disposed || end.kind === 'disposed') return { kind: 'unavailable' };
		const settlement = settle(end, operation, this.#snapshot.integrations);
		const { code, diagnosis } = settlement;
		// An explain failure is the diagnosis's own result, not a page failure.
		const pageFailure = diagnosis === undefined ? code : null;
		this.#publish({
			...(diagnosis === undefined ? {} : { diagnosis }),
			failureCode: pageFailure,
			failureIntegrationId: pageFailure === null ? null : integrationId,
			integrations: settlement.integrations,
			pending: false,
			pendingIntegrationId: null,
		});
		const snapshot = this.#protocolSnapshot();
		return code === null
			? { kind: 'completed', snapshot }
			: { code, integrationId, kind: 'failed', snapshot };
	}

	/** Starts one operation and polls it to its report; past the deadline it asks the job to stop. */
	async #runJob(operation: JobOperation): Promise<JobEnd> {
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

	#protocolSnapshot(): AgentIntegrationManagementSnapshot {
		return {
			integrations: this.#snapshot.integrations,
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

function operationTarget(operation: Operation): AgentIntegrationId | null {
	switch (operation.kind) {
		case 'change':
			return operation.mutation.integrationId;
		case 'explain':
			return operation.integrationId;
		case 'refresh':
			return null;
	}
}

function jobOperation(operation: Operation): JobOperation {
	switch (operation.kind) {
		case 'change':
			return mutationOperation(operation.mutation);
		case 'explain':
			return { kind: 'explain', provider: operation.integrationId };
		case 'refresh':
			return { kind: 'list' };
	}
}

function settle(end: SettledEnd, operation: Operation, rows: Rows): Settlement {
	switch (operation.kind) {
		case 'change':
			return settleMutation(end, operation.mutation, rows);
		case 'explain':
			return settleExplain(end, operation.integrationId, rows);
		case 'refresh':
			return settleRefresh(end, rows);
	}
}

// One literal per direction: a combined `kind` over every provider exceeds the compiler's
// discriminated-union expansion limit.
function mutationOperation(mutation: Mutation): JobOperation {
	return mutation.direction === 'enable'
		? { kind: 'enable', provider: mutation.integrationId }
		: { kind: 'disable', provider: mutation.integrationId };
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
	return { code: readFailure(end), integrations: rows };
}

/** A diagnosis also refreshes its Agent's row; one that did not finish leaves the rows as they were. */
function settleExplain(end: SettledEnd, integrationId: AgentIntegrationId, rows: Rows): Settlement {
	if (
		end.kind === 'completed' &&
		end.report.kind === 'explain' &&
		end.report.item.integrationId === integrationId
	) {
		const { item } = end.report;
		return {
			code: null,
			diagnosis: {
				explanation: explanation(end.report, integrationId),
				integrationId,
				kind: 'explained',
			},
			integrations: rows.map((row) =>
				row.integrationId === integrationId ? reportedRow(item, integrationId) : row,
			),
		};
	}
	const code = readFailure(end);
	return { code, diagnosis: { code, integrationId, kind: 'failed' }, integrations: rows };
}

/** A read changes nothing, so a read that did not finish is a definite failure, never unverified. */
function readFailure(end: SettledEnd): FailureCode {
	// A Worker without a home directory, or one that ran out of time, read nothing.
	if (end.kind === 'deadline') return 'operation-failed';
	if (end.kind === 'completed' && end.report.kind === 'home-unavailable') return 'operation-failed';
	if (end.kind === 'failed' && end.reason === 'busy' && !end.started) return 'busy';
	return 'control-unavailable';
}

/** Keeps the facts a person can act on; paths and command arguments stay out of the snapshot. */
function explanation(
	report: ExplainReport,
	integrationId: AgentIntegrationId,
): AgentIntegrationExplanation {
	const { capsule, detail, item, plan } = report;
	let adapter: AgentIntegrationExplanation['adapter'] = 'not-ready';
	if (capsule.bindingFailure) adapter = 'binding-failed';
	else if (capsule.ready) adapter = 'ready';
	else if (capsule.sourceKind === 'absent') adapter = 'source-absent';
	return {
		adapter,
		detail: inspectionDetail(detail),
		item: reportedRow(item, integrationId),
		plan: plan.map((step): AgentIntegrationExplanation['plan'][number] => {
			switch (step.kind) {
				case 'capsule-publish':
					return { kind: 'publish-adapter' };
				case 'command':
					return { command: commandName(step.executable), kind: 'command' };
				case 'guarded-write':
					return { kind: 'write-entry' };
				case 'guarded-remove':
					return { kind: 'remove-entry' };
			}
		}),
		timedOut: item.timedOut,
	};
}

function inspectionDetail(detail: ExplainReport['detail']): AgentIntegrationExplanation['detail'] {
	switch (detail.kind) {
		case 'cli':
			return {
				kind: 'cli',
				marketplace: detail.marketplace ?? null,
				pluginPresent: detail.pluginPresent ?? null,
			};
		case 'codex':
			return {
				kind: 'codex',
				marketplace: detail.marketplace,
				pluginPresent: detail.pluginPresent,
			};
		case 'guarded':
			return { entry: detail.targetState, kind: 'guarded', sourceReady: detail.sourceReady };
		case 'plain':
			return { kind: 'plain' };
	}
}

/** The executable's file name, as the CLI's explain output names the command. */
function commandName(executable: string): string {
	const name = executable
		.split(/[\\/]/u)
		.filter((part) => part !== '')
		.pop();
	return name ?? executable;
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
	integrationId: AgentIntegrationId,
): AgentIntegrationSnapshotItem {
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

function uncertainRows(rows: Rows, integrationId: AgentIntegrationId): Rows {
	return rows.map((row) =>
		row.integrationId === integrationId ? { ...row, issue: null, state: 'unknown' } : row,
	);
}

function mutatedRows(rows: Rows, report: MutationReport, integrationId: AgentIntegrationId): Rows {
	const { observed } = report;
	if (observed?.integrationId === integrationId) {
		return rows.map((row) =>
			row.integrationId === integrationId ? reportedRow(observed, integrationId) : row,
		);
	}
	return report.effectBegun ? uncertainRows(rows, integrationId) : rows;
}
