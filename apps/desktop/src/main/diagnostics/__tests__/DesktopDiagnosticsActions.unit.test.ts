import { describe, expect, it, vi } from 'vitest';

import {
	DesktopDiagnosticsActions,
	type DesktopDiagnosticBundleOutcome,
	type DesktopDiagnosticsTarget,
} from '../DesktopDiagnosticsActions';

function target(
	path: string | undefined,
	outcome: Promise<DesktopDiagnosticBundleOutcome>,
): DesktopDiagnosticsTarget & {
	readonly createBundle: ReturnType<typeof vi.fn<DesktopDiagnosticsTarget['createBundle']>>;
	readonly reportFailure: ReturnType<typeof vi.fn<DesktopDiagnosticsTarget['reportFailure']>>;
	readonly revealBundle: ReturnType<typeof vi.fn<DesktopDiagnosticsTarget['revealBundle']>>;
} {
	return {
		chooseBundlePath: vi.fn<DesktopDiagnosticsTarget['chooseBundlePath']>(async () => path),
		createBundle: vi.fn<DesktopDiagnosticsTarget['createBundle']>(() => outcome),
		reportFailure: vi.fn<DesktopDiagnosticsTarget['reportFailure']>(async () => undefined),
		revealBundle: vi.fn<DesktopDiagnosticsTarget['revealBundle']>(),
	};
}

/** A result the test settles once it has seen the save waiting for it. */
function deferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	const settlers: ((value: Value) => void)[] = [];
	const promise = new Promise<Value>((resolve) => {
		settlers.push(resolve);
	});
	return {
		promise,
		resolve: (value) => {
			for (const settle of settlers) settle(value);
		},
	};
}

describe('DesktopDiagnosticsActions', () => {
	it('is hidden where the bundle command is unsupported', () => {
		const actions = new DesktopDiagnosticsActions(
			target('C:\\b.zip', Promise.resolve({ kind: 'created', path: 'C:\\b.zip' })),
			'darwin',
		);
		expect(actions.getSaveBundleAvailability()).toEqual({ kind: 'hidden' });
		expect(actions.invokeSaveBundle('menu')).toBe(false);
		actions.dispose();
	});

	it('creates nothing when the person cancels the save dialog', async () => {
		const cancelled = target(undefined, Promise.resolve({ kind: 'created', path: 'unused' }));
		const actions = new DesktopDiagnosticsActions(cancelled, 'win32');
		expect(actions.invokeSaveBundle('menu')).toBe(true);
		await vi.waitFor(() => expect(actions.getSaveBundleAvailability().kind).toBe('enabled'));
		expect(cancelled.createBundle).not.toHaveBeenCalled();
		actions.dispose();
	});

	it('runs one save at a time and reports how it ended', async () => {
		const outcome = deferred<DesktopDiagnosticBundleOutcome>();
		const pending = target('C:\\out\\bundle.zip', outcome.promise);
		const actions = new DesktopDiagnosticsActions(pending, 'win32');
		expect(actions.invokeSaveBundle('menu')).toBe(true);
		await vi.waitFor(() => expect(pending.createBundle).toHaveBeenCalledOnce());
		expect(actions.getSaveBundleAvailability().kind).toBe('disabled');
		expect(actions.invokeSaveBundle('menu')).toBe(false);
		outcome.resolve({ kind: 'created', path: 'C:\\out\\bundle.zip' });
		await vi.waitFor(() =>
			expect(pending.revealBundle).toHaveBeenCalledWith('C:\\out\\bundle.zip'),
		);
		expect(actions.getSaveBundleAvailability().kind).toBe('enabled');

		const failing = target('C:\\out\\b.zip', Promise.resolve({ code: 'publish', kind: 'failed' }));
		const second = new DesktopDiagnosticsActions(failing, 'win32');
		second.invokeSaveBundle('menu');
		await vi.waitFor(() => expect(failing.reportFailure).toHaveBeenCalledOnce());
		expect(failing.revealBundle).not.toHaveBeenCalled();
		actions.dispose();
		second.dispose();
	});
});
