import { describe, expect, it, vi } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandExecutionLookupResultSchema,
	SemanticCommandInvocationRequestSchema,
	SemanticCommandInvocationResultSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandOperationListSchema,
	SemanticCommandTerminalResultSchema,
	type SemanticCommandInvocationRequest,
} from '@orchbee/semantic-command-protocol';
import {
	decodeSemanticCommandLocalHttpResponse,
	semanticCommandLocalHttpRequests,
} from '@orchbee/semantic-command-protocol/local-http';
import { type WorkbenchDirectory, WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench/terminal-presentation';

import { DesktopSemanticCommandCatalog } from '../DesktopSemanticCommandCatalog';
import { DesktopSemanticCommandService } from '../DesktopSemanticCommandService';
import { createDesktopWorkbenchReadOperations } from '../createDesktopWorkbenchReadOperations';
import { handleDesktopSemanticCommandLocalHttpRequest } from '../handleDesktopSemanticCommandLocalHttpRequest';

const generation = SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:read');
const directory: WorkbenchDirectory = {
	available: true,
	git: null,
	id: 'directory-1',
	kind: 'folder',
	name: 'source',
	path: '/source',
	source: 'selected',
};

function createDocument(projectCount = 1) {
	let presentationSequence = 1;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: {
			createOperationId: () => terminalPresentationOperationId('read:operation:1'),
			createPresentationId: () =>
				terminalPresentationId(`read:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId('read:removal:1'),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('read:presentation:initial'),
			kind: 'managed' as const,
		},
	});
	for (let index = 0; index < projectCount; index += 1) {
		const result = runtime.transition({
			directory: { ...directory, id: `directory-${index + 1}`, path: `/source/${index + 1}` },
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: `Workspace ${index + 1}`,
			projectName: `Project ${index + 1}`,
			terminalTitle: 'Terminal',
			type: 'project-created',
		});
		if (result.outcome !== 'applied') throw new Error('Project fixture failed.');
	}
	return runtime.getSharedDocument(workbenchSharedDocumentRevision('4'));
}

function createService(getDocument: () => ReturnType<typeof createDocument> | null) {
	let execution = 0;
	return new DesktopSemanticCommandService(
		new DesktopSemanticCommandCatalog(
			createDesktopWorkbenchReadOperations({
				coordinatorGeneration: generation,
				cursorSecret: new Uint8Array(32).fill(3),
				getDocument,
			}),
		),
		{
			coordinatorGeneration: generation,
			createExecutionIdentity: () =>
				SemanticCommandExecutionIdentitySchema.parse(`desktop:execution:${++execution}`),
		},
	);
}

async function invokeAndRead(
	service: DesktopSemanticCommandService,
	invocation: SemanticCommandInvocationRequest,
) {
	const admission = service.invoke({ kind: 'local-user' }, invocation);
	if (admission.kind !== 'accepted') return admission;
	await Promise.resolve();
	const observed = service.get({
		kind: 'execution',
		executionIdentity: admission.execution.executionIdentity,
	});
	if (observed.kind !== 'found' || observed.execution.phase !== 'terminal') {
		throw new Error('Expected one terminal query execution.');
	}
	return SemanticCommandTerminalResultSchema.parse(observed.execution.result);
}

function request(index: number, operationId: string, input: unknown) {
	return SemanticCommandInvocationRequestSchema.parse({
		protocolVersion: 1,
		requestIdentity: `cli:read:${index}`,
		operationId: SemanticCommandOperationIdSchema.parse(operationId),
		operationSchemaVersion: 1,
		input,
	});
}

describe('Desktop Workbench read operation catalog', () => {
	it('admits exactly the four read operations and no terminal mutation', () => {
		const service = createService(() => createDocument());
		expect(service.list({ kind: 'local-user' }).operations.map((entry) => entry.id)).toEqual([
			'workbench.directory.list',
			'workbench.project.list',
			'workbench.terminal.list',
			'workbench.workspace.list',
		]);
		expect(
			service.describe(
				{ kind: 'local-user' },
				SemanticCommandOperationIdSchema.parse('workbench.terminal.create'),
			).kind,
		).toBe('operation-unavailable');
	});

	it('pages a canonical snapshot and rejects stale, mismatched, and forged cursors', async () => {
		let document = createDocument(3);
		const service = createService(() => document);
		const first = await invokeAndRead(service, request(1, 'workbench.project.list', { limit: 2 }));
		expect(first).toMatchObject({
			kind: 'completed',
			output: { items: [{ projectId: 'project-1' }, { projectId: 'project-2' }] },
		});
		if (first.kind !== 'completed' || typeof first.output !== 'object' || first.output === null) {
			throw new Error('Expected a page output.');
		}
		const cursor = Reflect.get(first.output, 'nextCursor');
		expect(typeof cursor).toBe('string');
		expect(
			await invokeAndRead(service, request(2, 'workbench.project.list', { limit: 2, cursor })),
		).toMatchObject({ kind: 'completed', output: { items: [{ projectId: 'project-3' }] } });

		document = { ...document, revision: workbenchSharedDocumentRevision('5') };
		expect(
			await invokeAndRead(service, request(3, 'workbench.project.list', { limit: 2, cursor })),
		).toEqual({ kind: 'blocked', code: 'page-cursor-stale' });
		expect(
			await invokeAndRead(
				service,
				request(4, 'workbench.directory.list', { projectId: 'project-1', cursor }),
			),
		).toEqual({ kind: 'blocked', code: 'page-cursor-invalid' });
	});

	it('fails closed for missing documents, targets, and unknown input fields', async () => {
		const missingDocument = createService(() => null);
		expect(await invokeAndRead(missingDocument, request(1, 'workbench.project.list', {}))).toEqual({
			kind: 'blocked',
			code: 'workbench-document-missing',
		});

		const service = createService(() => createDocument());
		expect(
			await invokeAndRead(
				service,
				request(2, 'workbench.directory.list', { projectId: 'missing' }),
			),
		).toEqual({ kind: 'blocked', code: 'project-not-found' });
		expect(
			service.invoke(
				{ kind: 'local-user' },
				request(3, 'workbench.project.list', { activeProject: true }),
			),
		).toMatchObject({ kind: 'rejected', code: 'input-invalid' });
	});

	it('serves exactly the four read operations through the strict local HTTP boundary', async () => {
		const service = createService(() => createDocument());
		const listed = SemanticCommandOperationListSchema.parse(
			localExchange(service, semanticCommandLocalHttpRequests.listOperations()),
		);
		expect(listed.operations.map((operation) => operation.id)).toEqual([
			'workbench.directory.list',
			'workbench.project.list',
			'workbench.terminal.list',
			'workbench.workspace.list',
		]);
		const cases = [
			['workbench.project.list', {}],
			['workbench.directory.list', { projectId: 'project-1' }],
			['workbench.workspace.list', { projectId: 'project-1' }],
			['workbench.terminal.list', { location: { kind: 'standalone' } }],
		] as const;
		for (const [index, [operationId, input]] of cases.entries()) {
			const invocation = request(index + 10, operationId, input);
			const admitted = SemanticCommandInvocationResultSchema.parse(
				localExchange(service, semanticCommandLocalHttpRequests.invoke(invocation)),
			);
			expect(admitted.kind).toBe('accepted');
			await vi.waitFor(() => {
				const observed = SemanticCommandExecutionLookupResultSchema.parse(
					localExchange(
						service,
						semanticCommandLocalHttpRequests.getExecution({
							kind: 'request',
							requestIdentity: invocation.requestIdentity,
						}),
					),
				);
				if (observed.kind !== 'found' || observed.execution.phase !== 'terminal') {
					throw new Error('Read operation has not settled yet.');
				}
				expect(observed.execution.result.kind).toBe('completed');
			});
		}
	});
});

function localExchange(service: DesktopSemanticCommandService, requestBytes: Uint8Array): unknown {
	const response = decodeSemanticCommandLocalHttpResponse(
		handleDesktopSemanticCommandLocalHttpRequest(service, requestBytes),
	);
	expect(response.status).toBe(200);
	return response.body;
}
