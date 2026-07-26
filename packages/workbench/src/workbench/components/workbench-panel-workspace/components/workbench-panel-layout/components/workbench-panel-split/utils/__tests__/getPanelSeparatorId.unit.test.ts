import { describe, expect, it } from 'vitest';

import { getPanelSeparatorId } from '../getPanelSeparatorId';

describe('getPanelSeparatorId', () => {
	it('names a separator from its stable adjacent child identities', () => {
		expect(getPanelSeparatorId('split-1', 'panel-1', 'panel-2')).toBe('split-1--panel-1--panel-2');
	});
});
