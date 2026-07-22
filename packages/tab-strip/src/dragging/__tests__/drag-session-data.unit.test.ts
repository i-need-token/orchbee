import { describe, expect, it } from 'vitest';
import { DragSessionData } from '../drag-session-data';

describe('DragSessionData', () => {
	it('is exercised by dragging-tabs-session behavior tests', () => {
		expect(DragSessionData).toBeTypeOf('function');
	});
});
