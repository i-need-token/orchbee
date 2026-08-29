import {
	SemanticCommandInvocationRequestSchema,
	type SemanticCommandExecutionReference,
} from '@orchbee/semantic-command-protocol';
import {
	encodeSemanticCommandLocalHttpResponse,
	inspectSemanticCommandLocalHttpRequest,
	localHttpError,
	routeSemanticCommandLocalHttpRequest,
	type SemanticCommandLocalHttpErrorCode,
} from '@orchbee/semantic-command-protocol/local-http';

import type { DesktopSemanticCommandService } from './DesktopSemanticCommandService';

const localUser = { kind: 'local-user' } as const;

export function handleDesktopSemanticCommandLocalHttpRequest(
	service: DesktopSemanticCommandService,
	bytes: Uint8Array,
): Uint8Array {
	const inspected = inspectSemanticCommandLocalHttpRequest(bytes);
	if (inspected.kind !== 'complete') {
		return errorResponse(inspected.kind === 'rejected' ? inspected.code : 'malformed-request');
	}
	const routed = routeSemanticCommandLocalHttpRequest(inspected.frame);
	if (routed.kind === 'rejected') return errorResponse(routed.code);
	const route = routed.route;
	switch (route.kind) {
		case 'status':
			return successResponse(service.status());
		case 'operation-list':
			return successResponse(service.list(localUser));
		case 'operation-describe':
			return successResponse(service.describe(localUser, route.operationId));
		case 'execution-invoke': {
			const request = SemanticCommandInvocationRequestSchema.safeParse(route.body);
			return request.success
				? successResponse(service.invoke(localUser, request.data))
				: errorResponse('malformed-request');
		}
		case 'request-get':
			return successResponse(
				service.get({ kind: 'request', requestIdentity: route.requestIdentity }),
			);
		case 'execution-get':
			return successResponse(service.get(executionReference(route.executionIdentity)));
		case 'execution-cancel':
			return successResponse(service.cancel(executionReference(route.executionIdentity)));
	}
}

function executionReference(
	executionIdentity: Extract<
		SemanticCommandExecutionReference,
		{ readonly kind: 'execution' }
	>['executionIdentity'],
): SemanticCommandExecutionReference {
	return { kind: 'execution', executionIdentity };
}

function successResponse(body: unknown): Uint8Array {
	return encodeSemanticCommandLocalHttpResponse(200, body);
}

function errorResponse(code: SemanticCommandLocalHttpErrorCode): Uint8Array {
	return encodeSemanticCommandLocalHttpResponse(errorStatus(code), localHttpError(code));
}

function errorStatus(code: SemanticCommandLocalHttpErrorCode): number {
	switch (code) {
		case 'method-not-allowed':
			return 405;
		case 'request-capacity-exceeded':
			return 413;
		case 'request-timeout':
			return 408;
		case 'route-not-found':
			return 404;
		case 'connection-capacity-exhausted':
			return 503;
		case 'response-capacity-exceeded':
			return 500;
		case 'content-type-required':
		case 'malformed-request':
			return 400;
	}
}
