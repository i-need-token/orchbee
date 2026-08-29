import { SemanticCommandInvocationRequestSchema } from '../execution-contract';
import {
	SemanticCommandExecutionIdentitySchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandRequestIdentitySchema,
} from '../identity-contract';
import type {
	SemanticCommandExecutionReference,
	SemanticCommandInvocationRequest,
} from '../execution-contract';
import type { SemanticCommandOperationId } from '../identity-contract';
import {
	type SemanticCommandLocalHttpErrorCode,
	type SemanticCommandLocalHttpRoute,
} from './local-http-contract';
import {
	encodeSemanticCommandLocalHttpRequest,
	type SemanticCommandLocalHttpRequestFrame,
} from './local-http-framing';

export type SemanticCommandLocalHttpRouteResult =
	| { readonly code: SemanticCommandLocalHttpErrorCode; readonly kind: 'rejected' }
	| { readonly kind: 'routed'; readonly route: SemanticCommandLocalHttpRoute };

export function routeSemanticCommandLocalHttpRequest(
	frame: SemanticCommandLocalHttpRequestFrame,
): SemanticCommandLocalHttpRouteResult {
	if (frame.method === 'GET' && frame.path === '/v1/status') {
		return noBody(frame, { kind: 'status' });
	}
	if (frame.method === 'GET' && frame.path === '/v1/operations') {
		return noBody(frame, { kind: 'operation-list' });
	}
	const segments = frame.path.split('/');
	if (
		frame.method === 'GET' &&
		segments.length === 4 &&
		segments[1] === 'v1' &&
		segments[2] === 'operations'
	) {
		const operationId = parsePathIdentity(segments[3], SemanticCommandOperationIdSchema);
		return operationId === null
			? rejected('route-not-found')
			: noBody(frame, { kind: 'operation-describe', operationId });
	}
	if (frame.method === 'POST' && frame.path === '/v1/executions') {
		const parsed = SemanticCommandInvocationRequestSchema.safeParse(frame.body);
		return parsed.success
			? { kind: 'routed', route: { kind: 'execution-invoke', body: parsed.data } }
			: rejected('malformed-request');
	}
	if (
		frame.method === 'GET' &&
		segments.length === 4 &&
		segments[1] === 'v1' &&
		segments[2] === 'requests'
	) {
		const requestIdentity = parsePathIdentity(segments[3], SemanticCommandRequestIdentitySchema);
		return requestIdentity === null
			? rejected('route-not-found')
			: noBody(frame, { kind: 'request-get', requestIdentity });
	}
	if (segments.length === 4 && segments[1] === 'v1' && segments[2] === 'executions') {
		const executionIdentity = parsePathIdentity(
			segments[3],
			SemanticCommandExecutionIdentitySchema,
		);
		return frame.method === 'GET' && executionIdentity !== null
			? noBody(frame, { kind: 'execution-get', executionIdentity })
			: rejected('route-not-found');
	}
	if (
		frame.method === 'POST' &&
		segments.length === 5 &&
		segments[1] === 'v1' &&
		segments[2] === 'executions' &&
		segments[4] === 'cancel'
	) {
		const executionIdentity = parsePathIdentity(
			segments[3],
			SemanticCommandExecutionIdentitySchema,
		);
		return executionIdentity === null || !isEmptyObject(frame.body)
			? rejected('route-not-found')
			: { kind: 'routed', route: { kind: 'execution-cancel', executionIdentity } };
	}
	return rejected('route-not-found');
}

export const semanticCommandLocalHttpRequests = Object.freeze({
	cancelExecution: (executionIdentity: string): Uint8Array =>
		encodeSemanticCommandLocalHttpRequest(
			'POST',
			`/v1/executions/${encodeURIComponent(executionIdentity)}/cancel`,
			{},
		),
	describeOperation: (operationId: SemanticCommandOperationId): Uint8Array =>
		encodeSemanticCommandLocalHttpRequest(
			'GET',
			`/v1/operations/${encodeURIComponent(operationId)}`,
		),
	getExecution: (reference: SemanticCommandExecutionReference): Uint8Array =>
		encodeSemanticCommandLocalHttpRequest(
			'GET',
			reference.kind === 'request'
				? `/v1/requests/${encodeURIComponent(reference.requestIdentity)}`
				: `/v1/executions/${encodeURIComponent(reference.executionIdentity)}`,
		),
	invoke: (request: SemanticCommandInvocationRequest): Uint8Array =>
		encodeSemanticCommandLocalHttpRequest('POST', '/v1/executions', request),
	listOperations: (): Uint8Array => encodeSemanticCommandLocalHttpRequest('GET', '/v1/operations'),
	readStatus: (): Uint8Array => encodeSemanticCommandLocalHttpRequest('GET', '/v1/status'),
});

interface IdentitySchema<Value> {
	safeParse(
		candidate: unknown,
	): { readonly data: Value; readonly success: true } | { readonly success: false };
}

function parsePathIdentity<Value>(
	segment: string | undefined,
	schema: IdentitySchema<Value>,
): Value | null {
	if (segment === undefined || segment.length === 0) return null;
	let decoded: string;
	try {
		decoded = decodeURIComponent(segment);
	} catch {
		return null;
	}
	if (encodeURIComponent(decoded) !== segment) return null;
	const parsed = schema.safeParse(decoded);
	return parsed.success ? parsed.data : null;
}

function noBody(
	frame: SemanticCommandLocalHttpRequestFrame,
	route: SemanticCommandLocalHttpRoute,
): SemanticCommandLocalHttpRouteResult {
	return frame.body === null ? { kind: 'routed', route } : rejected('malformed-request');
}

function rejected(code: SemanticCommandLocalHttpErrorCode): SemanticCommandLocalHttpRouteResult {
	return { kind: 'rejected', code };
}

function isEmptyObject(value: unknown): boolean {
	return (
		typeof value === 'object' &&
		value !== null &&
		!Array.isArray(value) &&
		Object.keys(value).length === 0
	);
}
