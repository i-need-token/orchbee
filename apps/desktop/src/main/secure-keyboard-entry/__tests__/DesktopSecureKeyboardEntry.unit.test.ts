import { describe, expect, it, vi } from 'vitest';

import {
	DesktopSecureKeyboardEntry,
	type DesktopSecureKeyboardEntryAdapter,
} from '../DesktopSecureKeyboardEntry';

describe('DesktopSecureKeyboardEntry', () => {
	it('toggles the macOS platform capability and releases it on blur or disposal', () => {
		const adapter = createAdapter();
		const secureEntry = new DesktopSecureKeyboardEntry('darwin', adapter);

		expect(secureEntry.getState()).toBe('inactive');
		expect(secureEntry.toggle()).toBe(true);
		expect(secureEntry.getState()).toBe('active');
		secureEntry.release();
		expect(secureEntry.getState()).toBe('inactive');
		expect(secureEntry.toggle()).toBe(true);
		secureEntry.dispose();
		expect(secureEntry.getState()).toBe('unavailable');
		expect(adapter.setEnabled).toHaveBeenNthCalledWith(1, true);
		expect(adapter.setEnabled).toHaveBeenNthCalledWith(2, false);
		expect(adapter.setEnabled).toHaveBeenNthCalledWith(3, true);
		expect(adapter.setEnabled).toHaveBeenNthCalledWith(4, false);
	});

	it('does not call the macOS-only capability on unsupported platforms', () => {
		const adapter = createAdapter();
		const secureEntry = new DesktopSecureKeyboardEntry('linux', adapter);

		expect(secureEntry.getState()).toBe('unsupported');
		expect(secureEntry.toggle()).toBe(false);
		secureEntry.release();
		secureEntry.dispose();
		expect(adapter.isEnabled).not.toHaveBeenCalled();
		expect(adapter.setEnabled).not.toHaveBeenCalled();
	});

	it('cannot enable after app deactivation and does not reacquire on activation', () => {
		const adapter = createAdapter();
		const secureEntry = new DesktopSecureKeyboardEntry('darwin', adapter);

		expect(secureEntry.toggle()).toBe(true);
		secureEntry.deactivate();
		expect(secureEntry.getState()).toBe('inactive');
		expect(secureEntry.toggle()).toBe(false);
		secureEntry.activate();
		expect(secureEntry.getState()).toBe('inactive');
		expect(adapter.setEnabled).toHaveBeenCalledTimes(2);
	});

	it('fails closed when enabling or verification fails', () => {
		const adapter = createAdapter();
		adapter.isEnabled.mockReturnValueOnce(false).mockImplementationOnce(() => {
			throw new Error('verification failed');
		});
		const secureEntry = new DesktopSecureKeyboardEntry('darwin', adapter);

		expect(secureEntry.toggle()).toBe(false);
		expect(secureEntry.getState()).toBe('unavailable');
		expect(adapter.setEnabled).toHaveBeenNthCalledWith(1, true);
		expect(adapter.setEnabled).toHaveBeenNthCalledWith(2, false);
		expect(secureEntry.toggle()).toBe(false);
	});
});

function createAdapter() {
	let enabled = false;
	return {
		isEnabled: vi.fn<DesktopSecureKeyboardEntryAdapter['isEnabled']>(() => enabled),
		setEnabled: vi.fn<DesktopSecureKeyboardEntryAdapter['setEnabled']>((nextEnabled) => {
			enabled = nextEnabled;
		}),
	};
}
