import { describe, expect, it, vi } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandExecutionLookupResultSchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandOperationListSchema,
	type SemanticCommandExecutionSnapshot,
	type SemanticCommandInvocationRequest,
} from '@orchbee/semantic-command-protocol';
import {
	decodeSemanticCommandLocalHttpResponse,
	semanticCommandLocalHttpRequests,
} from '@orchbee/semantic-command-protocol/local-http';
import { type WorkbenchDirectory, WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import {
	type WorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench/terminal-presentation';

import { DesktopSemanticCommandCatalog } from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';
import { createDesktopWorkbenchTerminalCreateOperation } from '../createDesktopWorkbenchTerminalCreateOperation';
import { handleDesktopSemanticCommandLocalHttpRequest } from '../handleDesktopSemanticCommandLocalHttpRequest';

const generation = SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:create');
const directory: WorkbenchDirectory = {
	available: true,
	git: null,
	id: 'directory-1',
	kind: 'folder',
	name: 'source',
	path: '/source',
	source: 'selected',
};

interface Deferred<Value> {
	readonly promise: Promise<Value>;
	resolve(value: Value): void;
}

interface TerminalExecutionLookupResult {
	readonly execution: Extract<SemanticCommandExecutionSnapshot, { readonly phase: 'terminal' }>;
	readonly kind: 'found';
}

function deferred<Value>(): Deferred<Value> {
	let resolver: ((value: Value) => void) | undefined;
	return {
		promise: new Promise((resolve) => {
			resolver = resolve;
		}),
		resolve(value) {
			if (resolver === undefined) throw new Error('Deferred value has no resolver.');
			resolver(value);
		},
	};
}

function initialDocument(): WorkbenchSharedDocument {
	let presentationSequence = 1;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: {
			createOperationId: () => terminalPresentationOperationId('fixture:operation:initial'),
			createPresentationId: () =>
				terminalPresentationId(`fixture:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId('fixture:removal:initial'),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('fixture:presentation:standalone'),
			kind: 'managed',
		},
	});
	const created = runtime.transition({
		directory,
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Workspace',
		projectName: 'Project',
		terminalTitle: 'Terminal',
		type: 'project-created',
	});
	if (created.outcome !== 'applied') throw new Error('Project fixture failed.');
	return runtime.getSharedDocument(workbenchSharedDocumentRevision('4'));
}

function request(index: number): SemanticCommandInvocationRequest {
	return SemanticCommandInvocationRequestSchema.parse({
		protocolVersion: 1,
		requestIdentity: `cli:create:${index}`,
		operationId: 'workbench.terminal.create',
		operationSchemaVersion: 1,
		input: {
			directoryId: 'directory-1',
			location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
		},
	});
}

function createHarness(
	options: {
		readonly beforeMutation?: Promise<void>;
		readonly afterCommit?: Promise<void>;
		readonly maxTerminalExecutions?: number;
	} = {},
) {
	let document = initialDocument();
	let executionSequence = 0;
	let presentationSequence = 10;
	const operation = createDesktopWorkbenchTerminalCreateOperation({
		createOperationId: () =>
			terminalPresentationOperationId(`desktop:create:operation:${presentationSequence}`),
		createPresentationId: () =>
			terminalPresentationId(`desktop:create:presentation:${presentationSequence++}`),
		createRemovalId: () =>
			terminalPresentationRemovalId(`desktop:create:removal:${presentationSequence}`),
		defaultCwd: '/system',
		initialPresentationId: terminalPresentationId('desktop:create:initial'),
		mutateCanonicalDocument: async (mutation, committed) => {
			await options.beforeMutation;
			const decision = mutation(document);
			if (decision.kind === 'rejected') return decision;
			document = decision.document;
			committed(document);
			await options.afterCommit;
			return { document, kind: 'committed' };
		},
		terminalTitle: 'Terminal',
	});
	return {
		document: () => document,
		service: new DesktopSemanticCommandService(new DesktopSemanticCommandCatalog([operation]), {
			coordinatorGeneration: generation,
			createExecutionIdentity: () =>
				SemanticCommandExecutionIdentitySchema.parse(
					`desktop:create:execution:${++executionSequence}`,
				),
			...(options.maxTerminalExecutions === undefined
				? {}
				: { maxTerminalExecutions: options.maxTerminalExecutions }),
		}),
	};
}

describe('Desktop Workbench Terminal-create operation', () => {
	it('admits one strict managed command and commits its exact target through local HTTP', async () => {
		const harness = createHarness();
		const listed = SemanticCommandOperationListSchema.parse(
			localExchange(harness.service, semanticCommandLocalHttpRequests.listOperations()),
		);
		expect(listed.operations.map((operation) => operation.id)).toEqual([
			'workbench.terminal.create',
		]);

		const invocation = request(1);
		const admitted = SemanticCommandInvocationResultSchema.parse(
			localExchange(harness.service, semanticCommandLocalHttpRequests.invoke(invocation)),
		);
		expect(admitted).toMatchObject({ kind: 'accepted', replayed: false });
		const terminal = await waitForTerminal(harness.service, invocation);
		expect(terminal.execution.result).toMatchObject({
			code: 'terminal-created',
			kind: 'completed',
			output: {
				intent: { kind: 'provision' },
				revision: '5',
				terminalContextId: 'terminal-context-2',
			},
		});
		const workspaceContext = harness
			.document()
			.terminalContexts.find((context) => context.terminalContextId === 'terminal-context-2');
		expect(workspaceContext?.tabCollection.tabs).toHaveLength(2);
		expect(harness.document().revision).toBe('5');
	});

	it('deduplicates the retained request through one durable document commit', async () => {
		const harness = createHarness();
		const invocation = request(1);
		const first = SemanticCommandInvocationResultSchema.parse(
			localExchange(harness.service, semanticCommandLocalHttpRequests.invoke(invocation)),
		);
		await waitForTerminal(harness.service, invocation);
		const replay = SemanticCommandInvocationResultSchema.parse(
			localExchange(harness.service, semanticCommandLocalHttpRequests.invoke(invocation)),
		);

		expect(first).toMatchObject({ kind: 'accepted', replayed: false });
		expect(replay).toMatchObject({
			kind: 'accepted',
			replayed: true,
			execution: { phase: 'terminal' },
		});
		expect(harness.document().revision).toBe('5');
	});

	it('cancels while queued before mutation and preserves a completion after durable commit', async () => {
		const beforeMutation = deferred<void>();
		const preCommit = createHarness({ beforeMutation: beforeMutation.promise });
		preCommit.service.invoke({ kind: 'local-user' }, request(1));
		expect(
			preCommit.service.cancel({ kind: 'request', requestIdentity: request(1).requestIdentity }),
		).toMatchObject({
			forwarded: true,
			kind: 'cancellation-observed',
		});
		beforeMutation.resolve();
		expect((await waitForTerminal(preCommit.service, request(1))).execution.result).toEqual({
			code: 'operation-cancelled',
			kind: 'cancelled',
		});
		expect(preCommit.document().revision).toBe('4');

		const afterCommit = deferred<void>();
		const postCommit = createHarness({ afterCommit: afterCommit.promise });
		postCommit.service.invoke({ kind: 'local-user' }, request(2));
		await vi.waitFor(() => expect(postCommit.document().revision).toBe('5'));
		expect(
			postCommit.service.cancel({ kind: 'request', requestIdentity: request(2).requestIdentity }),
		).toMatchObject({
			forwarded: false,
			kind: 'cancellation-observed',
		});
		afterCommit.resolve();
		expect((await waitForTerminal(postCommit.service, request(2))).execution.result.kind).toBe(
			'completed',
		);
	});

	it('evicts only terminal receipts without undoing committed Terminal intents', async () => {
		const harness = createHarness({ maxTerminalExecutions: 1 });
		for (const index of [1, 2]) {
			harness.service.invoke({ kind: 'local-user' }, request(index));
			await waitForTerminal(harness.service, request(index));
		}
		expect(
			harness.service.get({ kind: 'request', requestIdentity: request(1).requestIdentity }).kind,
		).toBe('execution-receipt-unavailable');
		expect(harness.document().revision).toBe('6');
		const workspaceContext = harness
			.document()
			.terminalContexts.find((context) => context.terminalContextId === 'terminal-context-2');
		expect(workspaceContext?.tabCollection.tabs).toHaveLength(3);
	});
});

async function waitForTerminal(
	service: DesktopSemanticCommandService,
	invocation: SemanticCommandInvocationRequest,
): Promise<TerminalExecutionLookupResult> {
	await vi.waitFor(() => {
		const observed = service.get({
			kind: 'request',
			requestIdentity: invocation.requestIdentity,
		});
		if (observed.kind !== 'found' || observed.execution.phase !== 'terminal') {
			throw new Error('Terminal-create execution has not settled.');
		}
	});
	const observed = SemanticCommandExecutionLookupResultSchema.parse(
		service.get({ kind: 'request', requestIdentity: invocation.requestIdentity }),
	);
	if (observed.kind !== 'found' || observed.execution.phase !== 'terminal') {
		throw new Error('Expected a terminal Terminal-create execution.');
	}
	return { execution: observed.execution, kind: 'found' };
}

function localExchange(service: DesktopSemanticCommandService, requestBytes: Uint8Array): unknown {
	const response = decodeSemanticCommandLocalHttpResponse(
		handleDesktopSemanticCommandLocalHttpRequest(service, requestBytes),
	);
	expect(response.status).toBe(200);
	return response.body;
}
