import { describe, expect, it } from 'vitest';
import { ProjectsPanelController } from '../ProjectsPanelController';

describe('ProjectsPanelController', () => {
	it('is the controller exercised by projects-panel behavior tests', () => {
		expect(ProjectsPanelController).toBeTypeOf('function');
	});
});
