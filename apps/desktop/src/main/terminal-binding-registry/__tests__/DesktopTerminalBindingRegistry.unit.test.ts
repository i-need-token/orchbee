import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { TerminalBindingIdSchema } from '../../recovery';
import { DesktopTerminalBindingRegistry } from '../DesktopTerminalBindingRegistry';

function registration(presentation: string, binding: string) {
	return {
		bindingId: TerminalBindingIdSchema.parse(binding),
		presentationId: terminalPresentationId(presentation),
	};
}

describe('DesktopTerminalBindingRegistry', () => {
	it('maintains exact forward and reverse identity indexes', () => {
		const registry = new DesktopTerminalBindingRegistry();
		const first = registration('presentation-1', 'binding-1');
		const second = registration('presentation-2', 'binding-2');

		expect(registry.register(first)).toEqual({ kind: 'registered', registration: first });
		expect(registry.register(second)).toEqual({ kind: 'registered', registration: second });
		expect(registry.getByPresentationId(first.presentationId)).toEqual(first);
		expect(registry.getByBindingId(second.bindingId)).toEqual(second);
		expect(registry.size).toBe(2);
	});

	it('treats the same association as idempotent', () => {
		const registry = new DesktopTerminalBindingRegistry();
		const target = registration('presentation-1', 'binding-1');

		const registered = registry.register(target);
		const repeated = registry.register({ ...target });

		expect(registered.kind).toBe('registered');
		expect(repeated).toEqual({ kind: 'existing', registration: target });
		expect(registry.size).toBe(1);
	});

	it('rejects either side of a conflicting association without mutation', () => {
		const registry = new DesktopTerminalBindingRegistry();
		const target = registration('presentation-1', 'binding-1');
		registry.register(target);

		expect(registry.register(registration('presentation-1', 'binding-2'))).toEqual({
			kind: 'rejected',
			reason: 'presentation-id-conflict',
		});
		expect(registry.register(registration('presentation-2', 'binding-1'))).toEqual({
			kind: 'rejected',
			reason: 'binding-id-conflict',
		});
		expect(registry.getSnapshot()).toEqual([target]);
	});

	it('removes only the exact expected association', () => {
		const registry = new DesktopTerminalBindingRegistry();
		const target = registration('presentation-1', 'binding-1');
		registry.register(target);

		expect(
			registry.remove(target.presentationId, TerminalBindingIdSchema.parse('binding-stale')),
		).toEqual({ kind: 'rejected', reason: 'stale-binding' });
		expect(registry.getSnapshot()).toEqual([target]);
		expect(registry.remove(target.presentationId, target.bindingId)).toEqual({
			kind: 'removed',
			registration: target,
		});
		expect(registry.remove(target.presentationId, target.bindingId)).toEqual({
			kind: 'unchanged',
		});
		expect(registry.size).toBe(0);
	});

	it('retains every association without a product count threshold', () => {
		const registry = new DesktopTerminalBindingRegistry();

		for (let index = 0; index < 300; index += 1) {
			const target = registration(`presentation-${index}`, `binding-${index}`);
			expect(registry.register(target).kind).toBe('registered');
		}

		expect(registry.size).toBe(300);
		expect(registry.getSnapshot()).toHaveLength(300);
	});
});
