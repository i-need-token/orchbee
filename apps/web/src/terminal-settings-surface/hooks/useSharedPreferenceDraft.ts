import type { UserPreferenceKey, UserPreferencesPatch } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import type { UserPreferencesControl } from '../../user-preferences';

/** Keep the source captured by the first edit. External observations cannot silently rebase a draft. */
export function useSharedPreferenceDraft(
	value: string,
	key: UserPreferenceKey,
	control: UserPreferencesControl | undefined,
): {
	readonly draft: string;
	change(next: string): void;
	submit(changes: UserPreferencesPatch, local: () => void): void;
} {
	const [draft, setDraft] = React.useState(value);
	const edit = React.useRef<{
		dirty: boolean;
		source: string | undefined;
		revision: number;
		pending: boolean;
	}>({ dirty: false, source: undefined, revision: 0, pending: false });
	React.useEffect(() => {
		if (!edit.current.dirty) setDraft(value);
	}, [value]);
	const change = (next: string): void => {
		if (!edit.current.dirty) edit.current.source = control?.getSnapshot().snapshot?.source;
		edit.current.dirty = true;
		edit.current.revision += 1;
		setDraft(next);
	};
	const submit = (changes: UserPreferencesPatch, local: () => void): void => {
		if (control === undefined) {
			local();
			edit.current.dirty = false;
			return;
		}
		if (!edit.current.dirty || edit.current.pending) return;
		const revision = edit.current.revision;
		const source = edit.current.source;
		edit.current.pending = true;
		void control
			.update(changes, source)
			.then((response) => {
				if (response.kind !== 'snapshot') return;
				edit.current.source = response.snapshot.source;
				if (revision !== edit.current.revision) return;
				edit.current.dirty = false;
				setDraft(String(response.snapshot.values[key]));
			})
			.finally(() => {
				edit.current.pending = false;
			});
	};
	return { draft, change, submit };
}
