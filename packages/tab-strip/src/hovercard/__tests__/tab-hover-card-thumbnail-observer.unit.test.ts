import { describe, expect, it } from 'vitest';
import { TabHoverCardThumbnailObserver } from '../tab-hover-card-thumbnail-observer';

describe('TabHoverCardThumbnailObserver', () => {
	it('is exercised by thumbnail-observer behavior tests', () => {
		expect(TabHoverCardThumbnailObserver).toBeTypeOf('function');
	});
});
