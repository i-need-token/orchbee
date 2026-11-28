import { describe, expect, it } from 'vitest';

import { FileSessionRegistry, MemoryFileSessionPort } from '../../file-session';
import { CodeEditorFileDocuments } from '../CodeEditorFileDocuments';

class FakeModel {
	public disposals = 0;
	public text: string;

	public constructor(text: string) {
		this.text = text;
	}

	public dispose(): void {
		this.disposals += 1;
	}
}

const WORKSPACE = 'workspace';
const FILE = { key: '/root/a.txt', path: '/root/a.txt' };

async function settle(): Promise<void> {
	for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
}

describe('CodeEditorFileDocuments', () => {
	it('grants a file once for its documents, reads it again as it changes, and lets go with the last', async () => {
		const port = new MemoryFileSessionPort();
		port.setRoot(WORKSPACE, '/root');
		port.writeFile(FILE.path, 'one\n');
		const sessions = new FileSessionRegistry(port);
		const documents = new CodeEditorFileDocuments<FakeModel, never>({
			lease: () => sessions.hold(WORKSPACE),
			models: {
				create: (content) => new FakeModel(content.text),
				update: (model, content) => {
					model.text = content.text;
				},
			},
		});

		const first = documents.open(FILE, { source: 'tree', text: false });
		const second = documents.open(FILE, { source: 'tree', text: false });
		expect(second.document).toBe(first.document);
		await settle();
		expect(port.grants()).toHaveLength(1);
		const shown = first.kind === 'text' ? first.document.getState() : null;
		if (shown?.kind !== 'text') throw new Error('Expected the file as text.');
		expect(shown.model.text).toBe('one\n');

		port.writeFile(FILE.path, 'two\n');
		await settle();
		const reloaded = first.kind === 'text' ? first.document.getState() : null;
		expect(reloaded?.kind === 'text' ? reloaded.model : null).toBe(shown.model);
		expect(shown.model.text).toBe('two\n');

		first.release();
		first.release();
		expect(port.openWorkspaces()).toEqual([WORKSPACE]);
		second.release();
		expect(shown.model.disposals).toBe(1);
		expect(port.openWorkspaces()).toEqual([]);
	});
});
