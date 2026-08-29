import { describe, expect, it } from 'vitest';

import {
	SemanticCommandOperationDescriptorSchema,
	SemanticCommandOperationIdSchema,
	SemanticCommandSchemaVersionSchema,
	SemanticCommandTerminalResultSchema,
} from '@orchbee/semantic-command-protocol';

import {
	DesktopSemanticCommandCatalog,
	type DesktopSemanticCommandOperation,
} from '../DesktopSemanticCommandCatalog';

const caller = { kind: 'local-user' } as const;
const testOperationId = SemanticCommandOperationIdSchema.parse('test.resource.execute');
const testDescriptor = SemanticCommandOperationDescriptorSchema.parse({
	id: testOperationId,
	schemaVersion: 1,
	kind: 'command',
	confirmation: 'noninteractive-allowed',
	retry: 'same-request-while-retained',
	input: { name: 'test.resource-input', schemaVersion: 1 },
	output: { name: 'test.resource-output', schemaVersion: 1 },
});
const completedResult = SemanticCommandTerminalResultSchema.parse({
	kind: 'completed',
	code: 'operation-completed',
	output: { value: 'done' },
});

function semanticCommandOperation(
	overrides: Partial<DesktopSemanticCommandOperation> = {},
): DesktopSemanticCommandOperation {
	return {
		authorize: () => true,
		canonicalizeInput: (input) => ({ input, kind: 'accepted' }),
		descriptor: testDescriptor,
		execute: async () => ({ effect: 'none', result: completedResult }),
		isVisibleTo: () => true,
		validateOutput: () => true,
		...overrides,
	};
}

describe('DesktopSemanticCommandCatalog', () => {
	it('lists only explicitly supplied visible operations in canonical identity order', () => {
		const earlier = semanticCommandOperation({
			descriptor: SemanticCommandOperationDescriptorSchema.parse({
				...testDescriptor,
				id: 'alpha.resource.read',
			}),
		});
		const hidden = semanticCommandOperation({ isVisibleTo: () => false });
		const catalog = new DesktopSemanticCommandCatalog([
			hidden,
			semanticCommandOperation({
				descriptor: SemanticCommandOperationDescriptorSchema.parse({
					...testDescriptor,
					id: 'zeta.resource.read',
				}),
			}),
			earlier,
		]);

		expect(catalog.list(caller).map(({ id }) => id)).toEqual([
			'alpha.resource.read',
			'zeta.resource.read',
		]);
		expect(catalog.describe(caller, testOperationId)).toBeNull();
	});

	it('fails closed when visibility policy throws', () => {
		const catalog = new DesktopSemanticCommandCatalog([
			semanticCommandOperation({
				isVisibleTo: () => {
					throw new Error('policy unavailable');
				},
			}),
		]);

		expect(catalog.list(caller)).toEqual([]);
		expect(catalog.resolve(caller, testOperationId, testDescriptor.schemaVersion)).toEqual({
			kind: 'operation-unavailable',
		});
	});

	it('distinguishes an unavailable operation from an unsupported schema version', () => {
		const catalog = new DesktopSemanticCommandCatalog([semanticCommandOperation()]);

		expect(catalog.resolve(caller, testOperationId, testDescriptor.schemaVersion).kind).toBe(
			'resolved',
		);
		expect(
			catalog.resolve(caller, testOperationId, SemanticCommandSchemaVersionSchema.parse(2)).kind,
		).toBe('operation-version-unsupported');
	});

	it('rejects duplicate operation identities', () => {
		expect(
			() =>
				new DesktopSemanticCommandCatalog([semanticCommandOperation(), semanticCommandOperation()]),
		).toThrow(/Duplicate semantic command operation/u);
	});
});
