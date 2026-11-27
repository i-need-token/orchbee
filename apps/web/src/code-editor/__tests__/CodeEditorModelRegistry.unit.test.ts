import { describe, expect, it, vi } from 'vitest';

import { CodeEditorModelRegistry } from '../CodeEditorModelRegistry';

class FakeModel {
	public disposals = 0;

	public dispose(): void {
		this.disposals += 1;
	}
}

describe('CodeEditorModelRegistry', () => {
	it('shares one entry among its holds and disposes of it after the last release', () => {
		const registry = new CodeEditorModelRegistry<FakeModel>();
		const create = vi.fn<() => FakeModel>(() => new FakeModel());
		const textTab = registry.hold('file', create);
		const diffTab = registry.hold('file', create);
		const otherFile = registry.hold('other', create);

		expect(diffTab.entry).toBe(textTab.entry);
		expect(create).toHaveBeenCalledTimes(2);

		textTab.release();
		textTab.release();
		expect(textTab.entry.disposals).toBe(0);

		diffTab.release();
		expect(textTab.entry.disposals).toBe(1);
		expect(otherFile.entry.disposals).toBe(0);

		const reopened = registry.hold('file', create);
		expect(reopened.entry).not.toBe(textTab.entry);
		diffTab.release();
		expect(reopened.entry.disposals).toBe(0);
	});
});
