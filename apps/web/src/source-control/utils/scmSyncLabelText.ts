import { sourceControlMessages, type IntlShape } from '@orchbee/i18n';

import type { ScmSyncItem } from '../model/scm-repository-row';

/** VS Code's `syncLabel`, `1↓ 2↑`, or only the incoming commits when the remote takes no pushes. */
export function scmSyncLabelText(intl: IntlShape, label: ScmSyncItem['label']): string | null {
	if (label === null) return null;
	if (label.ahead === null) {
		return intl.formatMessage(sourceControlMessages.syncIncoming, { behind: label.behind });
	}
	return intl.formatMessage(sourceControlMessages.syncBoth, {
		ahead: label.ahead,
		behind: label.behind,
	});
}
