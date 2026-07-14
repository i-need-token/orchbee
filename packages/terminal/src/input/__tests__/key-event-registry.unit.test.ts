import { describe, expect, it, vi } from 'vitest';
import { KeyEventRegistry } from '../key-event-registry';

const context = { alternateScreen: false, hasSelection: false, readOnly: false };

describe('KeyEventRegistry', () => {
	it('dispatches by descending priority and stops after consumption', () => {
		const registry = new KeyEventRegistry();
		const calls: string[] = [];
		registry.register({ id: 'low', priority: 1, handle: () => (calls.push('low'), true) });
		registry.register({ id: 'high', priority: 10, handle: () => (calls.push('high'), true) });
		expect(registry.dispatch({} as KeyboardEvent, context)).toBe(true);
		expect(calls).toEqual(['high']);
	});

	it('applies predicates and removes disposed handlers', () => {
		const registry = new KeyEventRegistry();
		const handle = vi.fn<() => boolean>(() => true);
		const disposable = registry.register({
			id: 'selection',
			when: (value) => value.hasSelection,
			handle,
		});
		expect(registry.dispatch({} as KeyboardEvent, context)).toBe(false);
		expect(registry.dispatch({} as KeyboardEvent, { ...context, hasSelection: true })).toBe(true);
		disposable.dispose();
		expect(registry.list()).toEqual([]);
	});

	it('rejects duplicate stable identifiers', () => {
		const registry = new KeyEventRegistry();
		registry.register({ id: 'duplicate', handle: () => false });
		expect(() => registry.register({ id: 'duplicate', handle: () => false })).toThrow(
			'already registered',
		);
	});

	it('removes handlers by ownership scope', () => {
		const registry = new KeyEventRegistry();
		registry.register({ id: 'surface', handle: () => false });
		registry.register({ id: 'application', scope: 'application', handle: () => false });
		registry.clearScope('surface');
		expect(registry.list().map((handler) => handler.id)).toEqual(['application']);
	});
});
