import { describe, expect, it } from 'vitest';
import { ContextMenuRegistry } from '../ContextMenuRegistry';
import type { TerminalInteractionContext } from '../../host';

const context: TerminalInteractionContext = {
	cell: null,
	selection: null,
	link: null,
	semanticZone: null,
	cwd: null,
	modifiers: { alt: false, control: false, meta: false, shift: false },
	mouseReporting: false,
};

describe('ContextMenuRegistry', () => {
	it('merges and orders serializable provider contributions', () => {
		const registry = new ContextMenuRegistry();
		const disposable = registry.register({
			provide: () => [
				{ kind: 'action', actionId: 'app.inspect', group: 'z-app', order: 1 },
				{ kind: 'action', actionId: 'terminal.paste', group: 'a-clipboard', order: 20 },
			],
		});

		expect(
			registry.provide(context, [
				{ kind: 'action', actionId: 'terminal.copy', group: 'a-clipboard', order: 10 },
			]),
		).toEqual([
			{ kind: 'action', actionId: 'terminal.copy', group: 'a-clipboard', order: 10 },
			{ kind: 'action', actionId: 'terminal.paste', group: 'a-clipboard', order: 20 },
			{ kind: 'action', actionId: 'app.inspect', group: 'z-app', order: 1 },
		]);

		disposable.dispose();
		expect(registry.provide(context)).toEqual([]);
		registry.dispose();
	});
});
