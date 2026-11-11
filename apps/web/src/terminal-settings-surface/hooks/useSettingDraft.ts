import * as React from 'react';

import type { UserSettingsFailure } from '../../user-settings';

interface SettingDraft {
	readonly draft: string;
	change(next: string): void;
}

/**
 * The text a field edits for one setting. It follows the file's value whenever that changes, and a
 * change the file refused puts the field back on the file's value, so the field never shows a value
 * the file does not hold once the edit has settled.
 */
export function useSettingDraft(value: string, failure: UserSettingsFailure | null): SettingDraft {
	const [draft, setDraft] = React.useState(value);
	React.useEffect(() => {
		setDraft(value);
	}, [value, failure]);
	return { change: setDraft, draft };
}
