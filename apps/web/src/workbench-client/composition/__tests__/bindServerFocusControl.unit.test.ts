// @vitest-environment jsdom
import { TerminalPresentationIdSchema } from '@orchbee/workbench-client-protocol';
import { expect, it, vi } from 'vitest';
import { bindServerFocusControl } from '../bindServerFocusControl';

it('targets only the focused Panel and cancels on blur, target change, modal and disposal', async () => {
	let focused = true;
	let target = TerminalPresentationIdSchema.parse('1'.repeat(32));
	let navigation = () => {};
	const activateFocus = vi.fn<(id: typeof target, initial: boolean, signal: AbortSignal) => void>();
	vi.spyOn(document, 'hasFocus').mockImplementation(() => focused);
	const binding = bindServerFocusControl({
		readTarget: () => target,
		subscribeNavigation: (listener) => {
			navigation = listener;
			return () => {
				navigation = () => {};
			};
		},
		projectionClient: { activateFocus },
		titleWindow: window,
		ownerDocument: document,
	});
	try {
		const first = activateFocus.mock.calls[0];
		expect(first?.[0]).toBe(target);
		navigation();
		expect(activateFocus).toHaveBeenCalledTimes(1);
		focused = false;
		window.dispatchEvent(new Event('blur'));
		expect(first?.[2].aborted).toBe(true);
		focused = true;
		window.dispatchEvent(new Event('focus'));
		expect(activateFocus.mock.calls[1]?.[1]).toBe(false);
		const previous = activateFocus.mock.calls[1];
		target = TerminalPresentationIdSchema.parse('2'.repeat(32));
		navigation();
		expect(previous?.[2].aborted).toBe(true);
		expect(activateFocus.mock.lastCall?.[0]).toBe(target);
		const dialog = document.createElement('div');
		dialog.role = 'dialog';
		document.body.append(dialog);
		await vi.waitFor(() => expect(activateFocus.mock.lastCall?.[2].aborted).toBe(true));
		dialog.remove();
		await vi.waitFor(() => expect(activateFocus.mock.lastCall?.[2].aborted).toBe(false));
	} finally {
		binding.dispose();
		vi.restoreAllMocks();
	}
	expect(activateFocus.mock.lastCall?.[2].aborted).toBe(true);
});
