import { describe, expect, it } from 'vitest';

import { SemanticCommandInvocationRequestSchema } from '../../execution-contract';
import {
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandOperationIdSchema,
} from '../../identity-contract';
import {
	encodeSemanticCommandLocalHttpRequest,
	inspectSemanticCommandLocalHttpRequest,
} from '../local-http-framing';
import {
	routeSemanticCommandLocalHttpRequest,
	semanticCommandLocalHttpRequests,
} from '../local-http-routing';

const invocation = SemanticCommandInvocationRequestSchema.parse({
	protocolVersion: 1,
	requestIdentity: 'cli:request:1',
	operationId: 'workbench.project.list',
	operationSchemaVersion: 1,
	input: { limit: 100 },
});

describe('semantic command local HTTP routing', () => {
	it('round-trips every exact route without admitting an arbitrary path', () => {
		const requests = [
			semanticCommandLocalHttpRequests.readStatus(),
			semanticCommandLocalHttpRequests.listOperations(),
			semanticCommandLocalHttpRequests.describeOperation(
				SemanticCommandOperationIdSchema.parse('workbench.project.list'),
			),
			semanticCommandLocalHttpRequests.invoke(invocation),
			semanticCommandLocalHttpRequests.getExecution({
				kind: 'request',
				requestIdentity: invocation.requestIdentity,
			}),
			semanticCommandLocalHttpRequests.getExecution({
				kind: 'execution',
				executionIdentity: SemanticCommandExecutionIdentitySchema.parse('desktop:execution:1'),
			}),
			semanticCommandLocalHttpRequests.cancelExecution(
				SemanticCommandExecutionIdentitySchema.parse('desktop:execution:1'),
			),
		];

		expect(requests.map(inspectAndRoute)).toMatchObject([
			{ kind: 'routed', route: { kind: 'status' } },
			{ kind: 'routed', route: { kind: 'operation-list' } },
			{ kind: 'routed', route: { kind: 'operation-describe' } },
			{ kind: 'routed', route: { kind: 'execution-invoke' } },
			{ kind: 'routed', route: { kind: 'request-get' } },
			{ kind: 'routed', route: { kind: 'execution-get' } },
			{ kind: 'routed', route: { kind: 'execution-cancel' } },
		]);

		expect(
			inspectAndRoute(encodeSemanticCommandLocalHttpRequest('GET', '/v1/files/private')),
		).toEqual({ kind: 'rejected', code: 'route-not-found' });
	});
});

function inspectAndRoute(request: Uint8Array) {
	const inspection = inspectSemanticCommandLocalHttpRequest(request);
	return inspection.kind === 'complete'
		? routeSemanticCommandLocalHttpRequest(inspection.frame)
		: inspection;
}
