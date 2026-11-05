import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type {
	WorkbenchSearchArgumentOption,
	WorkbenchSearchArgumentPicker,
	WorkbenchSearchArgumentRead,
} from '../model/workbench-search-result';

/** What an open picker shows: its options, or one disabled row saying why there are none yet. */
export type WorkbenchSearchArgumentChoices =
	| { readonly kind: 'options'; readonly options: readonly WorkbenchSearchArgumentOption[] }
	| { readonly kind: 'status'; readonly label: string };

interface SettledRead {
	readonly entry: string;
	/** Null when the read rejected. */
	readonly read: WorkbenchSearchArgumentRead | null;
}

/**
 * Reads a picker's options once each time the picker is entered, named by `entry`, and abandons the
 * read when the picker goes back, closes, or unmounts. The catalog may project a new picker object
 * while one is open; that never starts the read again.
 */
export function useWorkbenchSearchArgumentRead(
	picker: WorkbenchSearchArgumentPicker | null,
	entry: string | null,
): WorkbenchSearchArgumentChoices | null {
	const [settled, setSettled] = React.useState<SettledRead | null>(null);
	const read = useMemoizedFn((signal: AbortSignal) =>
		picker?.kind === 'read' ? picker.read(signal) : Promise.reject(new Error('No read picker.')),
	);
	const readEntry = picker?.kind === 'read' ? entry : null;

	React.useEffect(() => {
		if (readEntry === null) return undefined;
		const controller = new AbortController();
		const settle = (result: WorkbenchSearchArgumentRead | null) => {
			if (!controller.signal.aborted) setSettled({ entry: readEntry, read: result });
		};
		read(controller.signal).then(settle, () => settle(null));
		return () => {
			controller.abort();
			// A later entry into the same picker reads again rather than showing this answer.
			setSettled(null);
		};
	}, [read, readEntry]);

	if (picker === null) return null;
	if (picker.kind === 'options') return { kind: 'options', options: picker.options };
	if (settled === null || settled.entry !== readEntry) {
		return { kind: 'status', label: picker.reading };
	}
	if (settled.read === null) return { kind: 'status', label: picker.readFailed };
	return settled.read.kind === 'options'
		? { kind: 'options', options: settled.read.options }
		: { kind: 'status', label: settled.read.label };
}
