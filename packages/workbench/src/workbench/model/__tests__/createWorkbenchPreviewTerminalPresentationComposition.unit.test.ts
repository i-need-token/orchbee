import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../createWorkbenchPreviewTerminalPresentationComposition';

afterEach(() => vi.restoreAllMocks());

describe('createWorkbenchPreviewTerminalPresentationComposition', () => {
	it('preserves the stable initial identity and allocates fresh dynamic identities', () => {
		vi.spyOn(globalThis.crypto, 'randomUUID')
			.mockReturnValueOnce('00000000-0000-4000-8000-000000000001')
			.mockReturnValueOnce('00000000-0000-4000-8000-000000000002');
		const composition = createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('preview:initial'),
		);

		expect(composition).toMatchObject({
			initialPresentationId: 'preview:initial',
			kind: 'preview',
		});
		expect(composition.createPresentationId()).toBe('preview:00000000-0000-4000-8000-000000000001');
		expect(composition.createPresentationId()).toBe('preview:00000000-0000-4000-8000-000000000002');
	});
});
