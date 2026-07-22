import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import { useWorkbenchTabRename } from '../useWorkbenchTabRename';

function WorkbenchTabRenameHarness(): string {
	const rename = useWorkbenchTabRename(vi.fn());

	return JSON.stringify({
		canSubmit: rename.canSubmit,
		draftLabel: rename.draftLabel,
		isOpen: rename.isOpen,
		originalLabel: rename.originalLabel,
	});
}

describe('useWorkbenchTabRename', () => {
	it('starts with a closed dialog and no rename draft', () => {
		const markup = renderToStaticMarkup(createElement(WorkbenchTabRenameHarness));

		expect(markup).toBe(
			'{&quot;canSubmit&quot;:false,&quot;draftLabel&quot;:&quot;&quot;,&quot;isOpen&quot;:false,&quot;originalLabel&quot;:&quot;&quot;}',
		);
	});
});
