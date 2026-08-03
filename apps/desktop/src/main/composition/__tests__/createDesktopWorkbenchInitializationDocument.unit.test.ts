import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { createDesktopWorkbenchInitializationDocument } from '../createDesktopWorkbenchInitializationDocument';

describe('createDesktopWorkbenchInitializationDocument', () => {
	it('creates one provision intent when no recovered terminal exists', () => {
		const result = createDesktopWorkbenchInitializationDocument(options([]));
		const root = result.terminalContexts[0]?.panelWorkspace.tabs[0]?.root;

		expect(result.revision).toBe('0');
		expect(root).toMatchObject({
			primaryPresentation: {
				intent: { kind: 'provision', operationId: 'operation-1' },
				presentationId: 'presentation-initial',
				title: 'Terminal 1',
			},
		});
	});

	it('creates retain intents for every recovered terminal without a count ceiling', () => {
		const recovered = Array.from({ length: 300 }, (_, index) => ({
			launchCwd: index % 2 === 0 ? `/workspace/${index + 1}` : null,
			presentationId: terminalPresentationId(`presentation-recovered-${index + 1}`),
		}));
		const result = createDesktopWorkbenchInitializationDocument(options(recovered));

		expect(result.terminalContexts[0]?.panelWorkspace.tabs).toHaveLength(300);
		expect(result.terminalContexts[0]?.panelWorkspace.tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				intent: { kind: 'retain' },
				launchCwd: '/workspace/1',
				presentationId: 'presentation-recovered-1',
			},
		});
		expect(result.terminalContexts[0]?.panelWorkspace.tabs[1]?.root).toMatchObject({
			primaryPresentation: { launchCwd: '/workspace' },
		});
	});
});

function options(
	recoveredPresentations: readonly {
		readonly launchCwd: string | null;
		readonly presentationId: ReturnType<typeof terminalPresentationId>;
	}[],
) {
	let operationSequence = 0;
	let presentationSequence = 0;
	let removalSequence = 0;
	return {
		createOperationId: () => terminalPresentationOperationId(`operation-${++operationSequence}`),
		createPresentationId: () => terminalPresentationId(`presentation-${++presentationSequence}`),
		createRemovalId: () => terminalPresentationRemovalId(`removal-${++removalSequence}`),
		defaultCwd: '/workspace',
		initialPresentationId: terminalPresentationId('presentation-initial'),
		recoveredPresentations,
		terminalTitle: (sequence: number) => `Terminal ${sequence}`,
	};
}
