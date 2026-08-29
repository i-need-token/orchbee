import { describe, expect, it, vi } from 'vitest';

import {
	encodeSemanticCommandLocalHttpResponse,
	SemanticCommandLocalEndpointIdSchema,
	type SemanticCommandLocalEndpointId,
} from '@orchbee/semantic-command-protocol/local-http';

import type { SemanticCommandEndpointNativeClient } from '../semantic-command-endpoint-native-client';
import { connectLocalSemanticCommandCoordinator } from '../connectLocalSemanticCommandCoordinator';

const firstEndpoint = SemanticCommandLocalEndpointIdSchema.parse('11'.repeat(16));
const secondEndpoint = SemanticCommandLocalEndpointIdSchema.parse('22'.repeat(16));

function locator(endpointId: SemanticCommandLocalEndpointId, generation: string): Uint8Array {
	return new TextEncoder().encode(
		JSON.stringify({
			locatorVersion: 1,
			protocolVersion: 1,
			coordinatorGeneration: generation,
			endpointId,
		}),
	);
}

function createNativeClient(
	locators: ReadonlyMap<SemanticCommandLocalEndpointId, Uint8Array>,
	generations: ReadonlyMap<SemanticCommandLocalEndpointId, string>,
): SemanticCommandEndpointNativeClient {
	return {
		exchangeAuthenticated: vi.fn<SemanticCommandEndpointNativeClient['exchangeAuthenticated']>(
			async (_runtime, endpointId) => {
				const generation = generations.get(SemanticCommandLocalEndpointIdSchema.parse(endpointId));
				if (generation === undefined) throw new Error('Endpoint unavailable.');
				return encodeSemanticCommandLocalHttpResponse(200, {
					protocolVersion: 1,
					coordinatorGeneration: generation,
					kind: 'ready',
				});
			},
		),
		readLocator: vi.fn<SemanticCommandEndpointNativeClient['readLocator']>(
			(_runtime, endpointId) => {
				const value = locators.get(SemanticCommandLocalEndpointIdSchema.parse(endpointId));
				if (value === undefined) throw new Error('Locator unavailable.');
				return value;
			},
		),
	};
}

const discovery = (names: readonly string[]) => ({
	readRuntimeDirectory: vi.fn<(runtimeDirectory: string) => Promise<readonly string[]>>(
		async () => names,
	),
});

describe('connectLocalSemanticCommandCoordinator', () => {
	it('connects only after a private locator and live status agree on generation', async () => {
		const nativeClient = createNativeClient(
			new Map([[firstEndpoint, locator(firstEndpoint, 'desktop:generation:1')]]),
			new Map([[firstEndpoint, 'desktop:generation:1']]),
		);
		const result = await connectLocalSemanticCommandCoordinator(
			{ nativeClient, runtimeDirectory: '/run/user/501/orchbee' },
			discovery([`${firstEndpoint}.c.json`, 'ignored.txt']),
		);

		expect(result.kind).toBe('connected');
		if (result.kind !== 'connected') throw new Error('Expected one connected coordinator.');
		await expect(result.client.readStatus()).resolves.toMatchObject({ kind: 'ok' });
		expect(result.locator.endpointId).toBe(firstEndpoint);
	});

	it('fails closed for no live match, malformed locator, or generation mismatch', async () => {
		const cases = [
			createNativeClient(new Map(), new Map()),
			createNativeClient(
				new Map([[firstEndpoint, new TextEncoder().encode('{')]]),
				new Map([[firstEndpoint, 'desktop:generation:1']]),
			),
			createNativeClient(
				new Map([[firstEndpoint, locator(firstEndpoint, 'desktop:generation:1')]]),
				new Map([[firstEndpoint, 'desktop:generation:2']]),
			),
		];
		for (const nativeClient of cases) {
			await expect(
				connectLocalSemanticCommandCoordinator(
					{ nativeClient, runtimeDirectory: '/run/user/501/orchbee' },
					discovery([`${firstEndpoint}.c.json`]),
				),
			).resolves.toEqual({ kind: 'error', code: 'coordinator-unavailable' });
		}
	});

	it('reports ambiguity instead of selecting by recency or directory order', async () => {
		const nativeClient = createNativeClient(
			new Map([
				[firstEndpoint, locator(firstEndpoint, 'desktop:generation:1')],
				[secondEndpoint, locator(secondEndpoint, 'desktop:generation:2')],
			]),
			new Map([
				[firstEndpoint, 'desktop:generation:1'],
				[secondEndpoint, 'desktop:generation:2'],
			]),
		);

		await expect(
			connectLocalSemanticCommandCoordinator(
				{ nativeClient, runtimeDirectory: '/run/user/501/orchbee' },
				discovery([`${secondEndpoint}.c.json`, `${firstEndpoint}.c.json`]),
			),
		).resolves.toEqual({ kind: 'error', code: 'coordinator-ambiguous' });
	});

	it('uses only an exact validated selector when one is supplied', async () => {
		const nativeClient = createNativeClient(
			new Map([[secondEndpoint, locator(secondEndpoint, 'desktop:generation:2')]]),
			new Map([[secondEndpoint, 'desktop:generation:2']]),
		);
		const readRuntimeDirectory = vi.fn<(runtimeDirectory: string) => Promise<readonly string[]>>(
			async () => [`${firstEndpoint}.c.json`],
		);

		const result = await connectLocalSemanticCommandCoordinator(
			{
				endpointId: secondEndpoint,
				nativeClient,
				runtimeDirectory: '/run/user/501/orchbee',
			},
			{ readRuntimeDirectory },
		);
		expect(result.kind).toBe('connected');
		expect(readRuntimeDirectory).not.toHaveBeenCalled();
	});
});
