import { describe, expect, it } from 'vitest';

import { FileOverlayRegistry } from '../FileOverlayRegistry';

function openFile(registry: FileOverlayRegistry, workspaceId: string, path: string): void {
	registry.store(workspaceId).dispatch({
		input: { kind: 'file', resource: { key: path, path } },
		preview: false,
		type: 'file-opened',
	});
}

describe('FileOverlayRegistry', () => {
	it('keeps one overlay per Workspace and creates none for an event alone', () => {
		const registry = new FileOverlayRegistry(false);
		expect(registry.notify('w1', { type: 'terminal-replaced' })).toEqual([]);
		openFile(registry, 'w1', '/a');

		expect(registry.store('w1')).toBe(registry.store('w1'));
		expect(registry.store('w1').state.getState().mode.kind).toBe('open');
		expect(registry.store('w2').state.getState().mode.kind).toBe('closed');

		registry.notify('w1', { type: 'terminal-replaced' });
		expect(registry.store('w1').state.getState().mode.kind).toBe('pip');
	});

	it('shrinks the overlay whose terminal another Tab or Workspace replaces', () => {
		const registry = new FileOverlayRegistry(false);
		openFile(registry, 'w1', '/a');
		openFile(registry, 'w2', '/b');
		const mode = (workspaceId: string): string =>
			registry.store(workspaceId).state.getState().mode.kind;

		registry.showTerminal({ tabId: null, terminalContextId: null, workspaceId: 'w1' });
		registry.showTerminal({ tabId: 't1', terminalContextId: 'c1', workspaceId: 'w1' });
		expect(mode('w1')).toBe('open');

		registry.showTerminal({ tabId: 't2', terminalContextId: 'c1', workspaceId: 'w1' });
		expect(mode('w1')).toBe('pip');

		registry.store('w1').dispatch({ type: 'pip-restored' });
		registry.showTerminal({ tabId: 't3', terminalContextId: 'c2', workspaceId: 'w2' });
		expect(mode('w1')).toBe('pip');
		expect(mode('w2')).toBe('open');
	});

	it('tells every overlay, and every later one, whether left-expand is offered', () => {
		const registry = new FileOverlayRegistry(true);
		openFile(registry, 'w1', '/a');
		registry.notify('w1', { tier: 'left-expand', type: 'tier-chosen' });

		registry.setLeftExpandOffered(false);
		expect(registry.store('w1').state.getState().mode.tier).toBe('default');
		expect(registry.store('w2').state.getState().mode.leftExpandOffered).toBe(false);
	});

	it('drops the overlays of vanished Workspaces but keeps one holding unsaved tabs', () => {
		const registry = new FileOverlayRegistry(false);
		openFile(registry, 'saved', '/a');
		openFile(registry, 'unsaved', '/b');
		registry.notify('unsaved', { id: 'file:/b', type: 'unsaved-changed', unsaved: true });
		const unsaved = registry.store('unsaved');

		registry.retain(new Set());
		expect(registry.store('saved').state.getState().mode.kind).toBe('closed');
		expect(registry.store('unsaved')).toBe(unsaved);
	});

	it("starts each Workspace's overlay as the device last kept it, until the Workspace vanishes", () => {
		let stored: string | null = null;
		const storage = {
			getItem: () => stored,
			setItem: (_key: string, value: string) => {
				stored = value;
			},
		};
		const kept = '0123456789abcdef0123456789abcdef';
		const gone = 'fedcba9876543210fedcba9876543210';
		const before = new FileOverlayRegistry(true, storage);
		before.retain(new Set([kept, gone]));
		openFile(before, kept, '/a');
		before.notify(kept, { type: 'terminal-replaced' });
		openFile(before, gone, '/b');

		const after = new FileOverlayRegistry(true, storage);
		expect(after.store(kept).state.getState()).toMatchObject({
			mode: { kind: 'pip' },
			tabs: { activeId: 'file:/a' },
		});

		before.retain(new Set([kept]));
		const later = new FileOverlayRegistry(true, storage);
		expect(later.store(gone).state.getState().mode.kind).toBe('closed');
		expect(later.store(kept).state.getState().mode.kind).toBe('pip');
	});
});
