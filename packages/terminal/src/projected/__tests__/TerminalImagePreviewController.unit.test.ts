import { CanonicalUint64TextSchema, type CanonicalUint64Text } from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	TerminalImagePreviewController,
	type TerminalImagePreviewMembership,
	type TerminalImagePreviewPresentation,
} from '../TerminalImagePreviewController';

describe('TerminalImagePreviewController', () => {
	afterEach(() => vi.useRealTimers());

	it('starts exactly one request after the complete dwell', () => {
		vi.useFakeTimers();
		const request = vi.fn<PreviewRequest>(() => CanonicalUint64TextSchema.parse('1'));
		const controller = new TerminalImagePreviewController({
			cancel: vi.fn<PreviewCancel>(),
			present: vi.fn<PreviewPresent>(),
			request,
		});
		const target = membership('/tmp/a.png');

		controller.hover(target);
		controller.hover(target);
		vi.advanceTimersByTime(299);
		expect(request).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		expect(request).toHaveBeenCalledOnce();
		expect(request).toHaveBeenCalledWith(target);
		expect(controller.snapshot).toEqual({
			membership: target,
			phase: 'loading',
			requestSequence: '1',
		});
	});

	it('cancels replacement work and rejects its reordered completion', () => {
		vi.useFakeTimers();
		const cancel = vi.fn<PreviewCancel>();
		const present = vi.fn<PreviewPresent>();
		let sequence = 0;
		const controller = new TerminalImagePreviewController({
			cancel,
			present,
			request: () => CanonicalUint64TextSchema.parse(String(++sequence)),
		});
		const first = membership('/tmp/a.png');
		const second = membership('/tmp/b.webp', { column: 9, row: '4' });

		controller.hover(first);
		vi.advanceTimersByTime(300);
		controller.hover(second);
		expect(cancel).toHaveBeenCalledWith('1');
		vi.advanceTimersByTime(300);
		controller.complete(CanonicalUint64TextSchema.parse('1'), previewBytes('image/png'));
		expect(present).not.toHaveBeenCalled();
		controller.complete(CanonicalUint64TextSchema.parse('2'), previewBytes('image/webp'));
		expect(present).toHaveBeenCalledOnce();
		expect(present).toHaveBeenCalledWith({
			anchor: second.anchor,
			content: expect.any(ArrayBuffer),
			kind: 'visible',
			mediaType: 'image/webp',
			path: '/tmp/b.webp',
		});
	});

	it('hides visible content and rejects a duplicate result after retirement', () => {
		vi.useFakeTimers();
		const present = vi.fn<PreviewPresent>();
		const controller = new TerminalImagePreviewController({
			cancel: vi.fn<PreviewCancel>(),
			present,
			request: () => CanonicalUint64TextSchema.parse('1'),
		});

		controller.hover(membership('/tmp/a.png'));
		vi.advanceTimersByTime(300);
		controller.complete(CanonicalUint64TextSchema.parse('1'), previewBytes('image/png'));
		controller.retire();
		controller.complete(CanonicalUint64TextSchema.parse('1'), previewBytes('image/png'));

		expect(present.mock.calls).toEqual([
			[
				expect.objectContaining({
					kind: 'visible',
					mediaType: 'image/png',
					path: '/tmp/a.png',
				}),
			],
			[{ kind: 'hidden' }],
		]);
		expect(controller.snapshot).toEqual({ phase: 'idle' });
	});

	it('suppresses repeated failure for one hover membership', () => {
		vi.useFakeTimers();
		const request = vi.fn<PreviewRequest>(() => CanonicalUint64TextSchema.parse('1'));
		const controller = new TerminalImagePreviewController({
			cancel: vi.fn<PreviewCancel>(),
			present: vi.fn<PreviewPresent>(),
			request,
		});
		const target = membership('/tmp/a.avif');

		controller.hover(target);
		vi.advanceTimersByTime(300);
		controller.unavailable(CanonicalUint64TextSchema.parse('1'));
		controller.hover(target);
		vi.advanceTimersByTime(600);

		expect(request).toHaveBeenCalledOnce();
		expect(controller.snapshot).toEqual({ membership: target, phase: 'unavailable' });
		controller.retire();
		controller.hover(target);
		vi.advanceTimersByTime(300);
		expect(request).toHaveBeenCalledTimes(2);
	});

	it.each([
		{ phase: 'loading', transition: 'hover' },
		{ phase: 'visible', transition: 'hover' },
		{ phase: 'loading', transition: 'retire' },
		{ phase: 'visible', transition: 'retire' },
	] as const)(
		'keeps the hover admitted while $transition retires $phase work',
		({ phase, transition }) => {
			vi.useFakeTimers();
			const harness = createRetirementHarness(phase);
			const latest = membership('/tmp/latest.png');
			const reenter = () => harness.controller.hover(latest);
			if (phase === 'loading') harness.cancel.mockImplementationOnce(reenter);
			else harness.present.mockImplementationOnce(reenter);

			if (transition === 'hover') harness.controller.hover(membership('/tmp/superseded.png'));
			else harness.controller.retire();

			expect(harness.controller.snapshot).toEqual({ membership: latest, phase: 'waiting' });
			expect(vi.getTimerCount()).toBe(1);
			expect(harness.cancel).toHaveBeenCalledTimes(phase === 'loading' ? 1 : 0);
			expect(harness.present).toHaveBeenCalledTimes(phase === 'visible' ? 2 : 0);
			vi.advanceTimersByTime(300);
			expect(harness.request).toHaveBeenCalledTimes(2);
			expect(harness.request).toHaveBeenLastCalledWith(latest);
			harness.controller.complete(CanonicalUint64TextSchema.parse('1'), previewBytes('image/png'));
			expect(harness.controller.snapshot).toEqual({
				membership: latest,
				phase: 'loading',
				requestSequence: '2',
			});
			harness.controller.dispose();
		},
	);

	it.each(['loading', 'visible'] as const)(
		'commits disposal before retiring %s work can reenter hover',
		(phase) => {
			vi.useFakeTimers();
			const harness = createRetirementHarness(phase);
			const reenter = () => harness.controller.hover(membership('/tmp/reentrant.png'));
			if (phase === 'loading') harness.cancel.mockImplementationOnce(reenter);
			else harness.present.mockImplementationOnce(reenter);

			harness.controller.dispose();

			expect(harness.controller.snapshot).toEqual({ phase: 'disposed' });
			expect(vi.getTimerCount()).toBe(0);
			expect(harness.cancel).toHaveBeenCalledTimes(phase === 'loading' ? 1 : 0);
			expect(harness.present).toHaveBeenCalledTimes(phase === 'visible' ? 2 : 0);
			vi.advanceTimersByTime(300);
			expect(harness.request).toHaveBeenCalledOnce();
		},
	);

	it.each(['loading', 'visible'] as const)(
		'does not resume hover after %s retirement disposes its owner',
		(phase) => {
			vi.useFakeTimers();
			const harness = createRetirementHarness(phase);
			const reenter = () => harness.controller.dispose();
			if (phase === 'loading') harness.cancel.mockImplementationOnce(reenter);
			else harness.present.mockImplementationOnce(reenter);

			harness.controller.hover(membership('/tmp/superseded.png'));

			expect(harness.controller.snapshot).toEqual({ phase: 'disposed' });
			expect(vi.getTimerCount()).toBe(0);
			expect(harness.cancel).toHaveBeenCalledTimes(phase === 'loading' ? 1 : 0);
			expect(harness.present).toHaveBeenCalledTimes(phase === 'visible' ? 2 : 0);
			vi.advanceTimersByTime(300);
			expect(harness.request).toHaveBeenCalledOnce();
		},
	);

	it('disposal cancels loading and permanently rejects later hover', () => {
		vi.useFakeTimers();
		const cancel = vi.fn<PreviewCancel>();
		const request = vi.fn<PreviewRequest>(() => CanonicalUint64TextSchema.parse('1'));
		const controller = new TerminalImagePreviewController({
			cancel,
			present: vi.fn<PreviewPresent>(),
			request,
		});

		controller.hover(membership('/tmp/a.bmp'));
		vi.advanceTimersByTime(300);
		controller.dispose();
		controller.hover(membership('/tmp/b.bmp'));
		vi.advanceTimersByTime(300);

		expect(cancel).toHaveBeenCalledExactlyOnceWith('1');
		expect(request).toHaveBeenCalledOnce();
		expect(controller.snapshot).toEqual({ phase: 'disposed' });
	});
});

type PreviewCancel = (requestSequence: CanonicalUint64Text) => void;
type PreviewPresent = (presentation: TerminalImagePreviewPresentation) => void;
type PreviewRequest = (membership: TerminalImagePreviewMembership) => CanonicalUint64Text;

function createRetirementHarness(phase: 'loading' | 'visible') {
	const cancel = vi.fn<PreviewCancel>();
	const present = vi.fn<PreviewPresent>();
	let sequence = 0;
	const request = vi.fn<PreviewRequest>(() => CanonicalUint64TextSchema.parse(String(++sequence)));
	const controller = new TerminalImagePreviewController({ cancel, present, request });
	controller.hover(membership('/tmp/original.png'));
	vi.advanceTimersByTime(300);
	if (phase === 'visible') {
		controller.complete(CanonicalUint64TextSchema.parse('1'), previewBytes('image/png'));
	}
	return { cancel, controller, present, request };
}

function membership(
	path: string,
	context: Partial<TerminalImagePreviewMembership['context']> = {},
): TerminalImagePreviewMembership {
	return {
		anchor: { height: 18, left: 20, surfaceHeight: 400, surfaceWidth: 600, top: 40, width: 8 },
		context: {
			column: context.column ?? 2,
			generation: context.generation ?? '1',
			linkEpoch: context.linkEpoch ?? '3',
			revision: context.revision ?? '7',
			row: context.row ?? '2',
		},
		target: { kind: 'file', path },
	};
}

function previewBytes(mediaType: 'image/png' | 'image/webp') {
	return { content: new ArrayBuffer(4), mediaType };
}
