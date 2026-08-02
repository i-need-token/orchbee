import { CreationIdSchema, HostEpochSchema, TerminalIdSchema } from '@orchbee/terminal-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import {
	createTerminalBindingStoreDocument,
	findTerminalBinding,
	putTerminalBinding,
	removeTerminalBinding,
	TerminalBindingIdSchema,
	TerminalBindingSchema,
	TerminalBindingStoreFileSchema,
	type TerminalBinding,
} from '../terminal-binding-store';

describe('terminal binding store model', () => {
	it('preserves canonical order and unrelated records across targeted put and remove', () => {
		const first = createBinding(1);
		const second = createBinding(2);
		const document = createTerminalBindingStoreDocument([second, first]);
		expect(document.bindings.map((binding) => binding.bindingId)).toEqual([
			first.bindingId,
			second.bindingId,
		]);
		const requested = TerminalBindingSchema.parse({
			...second,
			state: { hostEpoch: HostEpochSchema.parse('aa'.repeat(16)), kind: 'create-requested' },
		});
		const updated = putTerminalBinding(document, requested);

		expect(findTerminalBinding(updated, first.bindingId)).toEqual(first);
		expect(findTerminalBinding(updated, second.bindingId)).toEqual(requested);
		expect(removeTerminalBinding(updated, second.bindingId)).toEqual({
			bindings: [first],
			version: 3,
		});
	});

	it('retains an exact durable identity while a binding is closing', () => {
		const binding = createBinding(1);
		const identity = {
			creationId: binding.creationId,
			hostEpoch: HostEpochSchema.parse('aa'.repeat(16)),
			terminalId: TerminalIdSchema.parse('bb'.repeat(16)),
		};
		const closing = TerminalBindingSchema.parse({
			...binding,
			state: { identity, kind: 'closing' },
		});

		expect(createTerminalBindingStoreDocument([closing])).toEqual({
			bindings: [closing],
			version: 3,
		});
	});

	it('rejects noncanonical order, duplicate durable identities, and foreign CreationIds', () => {
		const first = createBinding(1);
		const second = createBinding(2);
		expect(
			TerminalBindingStoreFileSchema.safeParse({ bindings: [second, first], version: 3 }).success,
		).toBe(false);
		expect(
			TerminalBindingStoreFileSchema.safeParse({
				bindings: [first, { ...second, creationId: first.creationId }],
				version: 3,
			}).success,
		).toBe(false);
		expect(
			TerminalBindingSchema.safeParse({
				...first,
				state: {
					identity: {
						creationId: second.creationId,
						hostEpoch: HostEpochSchema.parse('aa'.repeat(16)),
						terminalId: TerminalIdSchema.parse('bb'.repeat(16)),
					},
					kind: 'retained',
				},
			}).success,
		).toBe(false);
	});

	it('rejects obsolete single-binding documents instead of migrating them', () => {
		expect(
			TerminalBindingStoreFileSchema.safeParse({ binding: createBinding(1), version: 1 }).success,
		).toBe(false);
	});

	it('admits collections beyond the former file-size-derived count ceiling', () => {
		const bindings = Array.from({ length: 257 }, (_, index) => createBinding(index + 1));
		const document = createTerminalBindingStoreDocument(bindings);

		expect(document.bindings).toHaveLength(257);
		expect(JSON.stringify(document).length).toBeGreaterThan(128 * 1024);
	});
});

function createBinding(index: number): TerminalBinding {
	const suffix = index.toString(16).padStart(32, '0');
	return TerminalBindingSchema.parse({
		bindingId: TerminalBindingIdSchema.parse(`binding-${index.toString().padStart(4, '0')}`),
		creationId: CreationIdSchema.parse(suffix),
		launch: {
			arguments: [],
			cellPixelHeight: 16,
			cellPixelWidth: 8,
			columns: 80,
			currentDirectory: '/users/orchbee',
			environment: [{ name: 'TERM', value: 'xterm-256color' }],
			executable: '/bin/sh',
			rows: 24,
			terminalProfile: createDefaultTerminalProfile(),
		},
		presentationId: terminalPresentationId(`presentation-${index.toString().padStart(4, '0')}`),
		state: { kind: 'planned' },
	});
}
