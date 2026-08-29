import { describe, expect, it } from 'vitest';

import {
	SemanticCommandCoordinatorGenerationSchema,
	SemanticCommandOperationIdSchema,
} from '@orchbee/semantic-command-protocol';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';

import { DesktopSemanticCommandPageCursor } from '../DesktopSemanticCommandPageCursor';

const context = {
	coordinatorGeneration: SemanticCommandCoordinatorGenerationSchema.parse('desktop:generation:1'),
	operationId: SemanticCommandOperationIdSchema.parse('workbench.project.list'),
	revision: workbenchSharedDocumentRevision('7'),
	target: 'projects',
};

describe('DesktopSemanticCommandPageCursor', () => {
	it('binds an opaque offset to operation, target, generation, and revision', () => {
		const cursor = new DesktopSemanticCommandPageCursor(new Uint8Array(32).fill(7));
		const encoded = cursor.create(context, 100);

		expect(cursor.resolve(encoded, context)).toEqual({ kind: 'resolved', offset: 100 });
		expect(cursor.resolve(encoded, { ...context, target: 'project:other' })).toEqual({
			kind: 'invalid',
		});
		expect(
			cursor.resolve(encoded, { ...context, revision: workbenchSharedDocumentRevision('8') }),
		).toEqual({ kind: 'stale' });
		expect(cursor.resolve(`${encoded.slice(0, -1)}x`, context)).toEqual({ kind: 'invalid' });
	});

	it('starts the first page at zero and rejects a weak cursor secret', () => {
		const cursor = new DesktopSemanticCommandPageCursor(new Uint8Array(16).fill(1));
		expect(cursor.resolve(undefined, context)).toEqual({ kind: 'resolved', offset: 0 });
		expect(() => new DesktopSemanticCommandPageCursor(new Uint8Array(15))).toThrow(/16 bytes/u);
	});
});
