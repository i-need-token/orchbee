import path from 'node:path';

import {
	SemanticCommandExecutionLookupResultSchema,
	type SemanticCommandExecutionReference,
	type SemanticCommandInvocationRequest,
	type SemanticCommandOperationId,
} from '@orchbee/semantic-command-protocol';
import {
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES,
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES,
	SemanticCommandLocalEndpointIdSchema,
	decodeSemanticCommandLocalHttpResponse,
	semanticCommandLocalHttpRequests,
	type SemanticCommandLocalEndpointId,
} from '@orchbee/semantic-command-protocol/local-http';

import type { SemanticCommandClientPort } from '../client';
import type { SemanticCommandEndpointNativeClient } from './semantic-command-endpoint-native-client';

const DEFAULT_EXCHANGE_TIMEOUT_MILLISECONDS = 5_000;
const MAX_EXCHANGE_TIMEOUT_MILLISECONDS = 60_000;
const RESPONSE_CAPACITY =
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES +
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES;

export interface LocalHttpSemanticCommandClientPortOptions {
	readonly endpointId: SemanticCommandLocalEndpointId;
	readonly nativeClient: SemanticCommandEndpointNativeClient;
	readonly runtimeDirectory: string;
	readonly timeoutMilliseconds?: number;
}

export class LocalHttpSemanticCommandClientPort implements SemanticCommandClientPort {
	readonly #endpointId: SemanticCommandLocalEndpointId;
	readonly #nativeClient: SemanticCommandEndpointNativeClient;
	readonly #runtimeDirectory: string;
	readonly #timeoutMilliseconds: number;

	constructor(options: LocalHttpSemanticCommandClientPortOptions) {
		if (!path.isAbsolute(options.runtimeDirectory)) {
			throw new Error('Semantic command runtime directory must be absolute.');
		}
		const timeoutMilliseconds =
			options.timeoutMilliseconds ?? DEFAULT_EXCHANGE_TIMEOUT_MILLISECONDS;
		if (
			!Number.isSafeInteger(timeoutMilliseconds) ||
			timeoutMilliseconds <= 0 ||
			timeoutMilliseconds > MAX_EXCHANGE_TIMEOUT_MILLISECONDS
		) {
			throw new Error('Semantic command exchange timeout is invalid.');
		}
		this.#endpointId = SemanticCommandLocalEndpointIdSchema.parse(options.endpointId);
		this.#nativeClient = options.nativeClient;
		this.#runtimeDirectory = options.runtimeDirectory;
		this.#timeoutMilliseconds = timeoutMilliseconds;
	}

	async cancelExecution(reference: SemanticCommandExecutionReference): Promise<unknown> {
		if (reference.kind === 'execution') {
			return this.#exchange(
				semanticCommandLocalHttpRequests.cancelExecution(reference.executionIdentity),
			);
		}
		const lookup = SemanticCommandExecutionLookupResultSchema.safeParse(
			await this.getExecution(reference),
		);
		if (!lookup.success) throw new Error('Semantic command receipt response is invalid.');
		if (lookup.data.kind === 'execution-receipt-unavailable') return lookup.data;
		return this.#exchange(
			semanticCommandLocalHttpRequests.cancelExecution(lookup.data.execution.executionIdentity),
		);
	}

	describeOperation(operationId: SemanticCommandOperationId): Promise<unknown> {
		return this.#exchange(semanticCommandLocalHttpRequests.describeOperation(operationId));
	}

	getExecution(reference: SemanticCommandExecutionReference): Promise<unknown> {
		return this.#exchange(semanticCommandLocalHttpRequests.getExecution(reference));
	}

	invoke(request: SemanticCommandInvocationRequest): Promise<unknown> {
		return this.#exchange(semanticCommandLocalHttpRequests.invoke(request));
	}

	listOperations(): Promise<unknown> {
		return this.#exchange(semanticCommandLocalHttpRequests.listOperations());
	}

	readStatus(): Promise<unknown> {
		return this.#exchange(semanticCommandLocalHttpRequests.readStatus());
	}

	async #exchange(request: Uint8Array): Promise<unknown> {
		const response = await this.#nativeClient.exchangeAuthenticated(
			this.#runtimeDirectory,
			this.#endpointId,
			request,
			RESPONSE_CAPACITY,
			this.#timeoutMilliseconds,
		);
		const decoded = decodeSemanticCommandLocalHttpResponse(response);
		if (decoded.status !== 200) {
			throw new Error('Semantic command endpoint rejected the local request.');
		}
		return decoded.body;
	}
}
