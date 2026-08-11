import { describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../createWorkbenchPreviewTerminalPresentationComposition';

describe('createWorkbenchPreviewTerminalPresentationComposition', () => {
	it('preserves the stable initial identity and allocates fresh dynamic identities', () => {
		const createPresentationId = vi
			.fn<() => ReturnType<typeof terminalPresentationId>>()
			.mockReturnValueOnce(terminalPresentationId('preview:dynamic:1'))
			.mockReturnValueOnce(terminalPresentationId('preview:dynamic:2'));
		const composition = createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('preview:initial'),
			createPresentationId,
		);

		expect(composition).toMatchObject({
			initialPresentationId: 'preview:initial',
			kind: 'preview',
		});
		expect(composition.createPresentationId()).toBe('preview:dynamic:1');
		expect(composition.createPresentationId()).toBe('preview:dynamic:2');
		expect(createPresentationId).toHaveBeenCalledTimes(2);
	});
});
