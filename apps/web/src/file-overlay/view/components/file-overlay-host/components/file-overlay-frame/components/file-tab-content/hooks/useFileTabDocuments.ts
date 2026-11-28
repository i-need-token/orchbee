import { useMemoizedFn } from '@orchbee/foundation/react';
import { resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import { useEffect, useState, useSyncExternalStore } from 'react';

import { loadCodeEditor } from '../../../../../../../../../code-editor';
import type { FileTabContents, FileTabShown } from '../../../../../../../../workspace-stores';

type CodeEditorRuntime = Awaited<ReturnType<typeof loadCodeEditor>>;

/** The code editor's chunk as the content area waits for it. */
type CodeEditorLoad =
	| { readonly kind: 'failed' }
	| { readonly kind: 'loading' }
	| { readonly kind: 'ready'; readonly runtime: CodeEditorRuntime };

interface FileTabDocuments {
	readonly load: CodeEditorLoad;
	/** Loads the code editor again after its chunk failed to load. */
	readonly retry: () => void;
	/** The active tab's file and document; null without tabs or until the documents are attached. */
	readonly shown: FileTabShown | null;
}

const LOADING: CodeEditorLoad = { kind: 'loading' };
const FAILED: CodeEditorLoad = { kind: 'failed' };
const subscribeToNothing = (): (() => void) => () => undefined;
const nothingShown = (): null => null;

/**
 * Loads the code editor for the content area, gives the tabs their documents once it has loaded,
 * and follows what the active tab shows. Without a file session to read from, nothing loads.
 */
export function useFileTabDocuments(contents: FileTabContents | null): FileTabDocuments {
	const locale = resolveOrchbeeLocale([useOrchbeeIntl().locale]);
	const [load, setLoad] = useState<CodeEditorLoad>(LOADING);
	const [attempt, setAttempt] = useState(0);
	const reading = contents !== null;
	useEffect(() => {
		if (!reading) return undefined;
		let current = true;
		void loadCodeEditor(locale).then(
			(runtime) => {
				if (current) setLoad({ kind: 'ready', runtime });
			},
			() => {
				if (current) setLoad(FAILED);
			},
		);
		return () => {
			current = false;
		};
	}, [attempt, locale, reading]);
	const runtime = load.kind === 'ready' ? load.runtime : null;
	// The documents stay attached for as long as the Workspace's tabs live, not only while they show.
	useEffect(() => {
		if (runtime === null || contents === null) return;
		contents.attach((lease) => runtime.createCodeEditorFileDocuments(runtime, lease));
	}, [contents, runtime]);
	const shown = useSyncExternalStore(
		contents?.subscribe ?? subscribeToNothing,
		contents?.getShown ?? nothingShown,
		contents?.getShown ?? nothingShown,
	);
	const retry = useMemoizedFn(() => {
		setLoad(LOADING);
		setAttempt((count) => count + 1);
	});
	return { load, retry, shown };
}
