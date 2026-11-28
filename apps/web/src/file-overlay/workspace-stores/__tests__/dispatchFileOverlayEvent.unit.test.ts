import { describe, expect, it, vi } from 'vitest';

import type { FileTab } from '../../model';
import { dispatchFileOverlayEvent, type FileTabContentPort } from '../dispatchFileOverlayEvent';
import { FileOverlayRegistry } from '../FileOverlayRegistry';

/** An overlay whose tabs all have unsaved edits, closing every tab and answering Save All. */
function savingAll(save: FileTabContentPort['save']) {
	const store = new FileOverlayRegistry(false).store('w1');
	for (const path of ['/a', '/b']) {
		store.dispatch({
			input: { kind: 'file', resource: { key: path, path } },
			preview: false,
			type: 'file-opened',
		});
		store.dispatch({ id: `file:${path}`, type: 'unsaved-changed', unsaved: true });
	}
	const content = {
		closed: vi.fn<(tabs: readonly FileTab[]) => void>(),
		revert: vi.fn<(tabs: readonly FileTab[]) => void>(),
		save: vi.fn<FileTabContentPort['save']>(save),
	};
	const ports = { content, focusTerminal: vi.fn<() => void>() };
	dispatchFileOverlayEvent(store, { target: { kind: 'all' }, type: 'close-requested' }, ports);
	dispatchFileOverlayEvent(store, { decision: 'save', type: 'close-decided' }, ports);
	const settled = (): Promise<void> =>
		vi.waitFor(() => {
			expect(store.state.getState().tabs.pendingClose).toBeNull();
		});
	const openIds = (): readonly string[] => store.state.getState().tabs.tabs.map((tab) => tab.id);
	return { content, openIds, ports, settled, store };
}

describe('running the file overlay effects', () => {
	it('closes the tabs that saved and keeps open the ones whose save failed', async () => {
		const { content, openIds, settled } = savingAll(() => Promise.resolve(['file:/b']));

		expect(content.save.mock.calls.map(([tabs]) => tabs.map((tab) => tab.id))).toEqual([
			['file:/a', 'file:/b'],
		]);
		await settled();

		expect(openIds()).toEqual(['file:/b']);
		expect(content.closed.mock.calls.map(([tabs]) => tabs.map((tab) => tab.id))).toEqual([
			['file:/a'],
		]);
		expect(content.revert).not.toHaveBeenCalled();
	});

	it('keeps every tab when the save itself fails, so the next close asks again', async () => {
		const { content, openIds, ports, settled, store } = savingAll(() =>
			Promise.reject(new Error('The disk is full')),
		);
		await settled();

		expect(openIds()).toEqual(['file:/a', 'file:/b']);
		expect(content.closed).not.toHaveBeenCalled();
		dispatchFileOverlayEvent(store, { target: { kind: 'all' }, type: 'close-requested' }, ports);
		expect(store.state.getState().tabs.pendingClose?.phase).toBe('deciding');
	});
});
