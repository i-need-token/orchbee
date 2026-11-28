import { sourceControlMessages, type IntlShape } from '@orchbee/i18n';

import type { ScmSyncTooltip } from '../model/scm-repository-row';

/** VS Code's sync tooltip in words: what a sync would pull and push, or what runs; null for none. */
export function scmSyncTooltipText(intl: IntlShape, tooltip: ScmSyncTooltip): string | null {
	switch (tooltip.kind) {
		case 'committing':
			return intl.formatMessage(sourceControlMessages.syncCommitting);
		case 'none':
			return null;
		case 'publish':
			return intl.formatMessage(sourceControlMessages.syncPublish);
		case 'pull':
			return intl.formatMessage(sourceControlMessages.syncPull, {
				count: tooltip.behind,
				upstream: tooltip.upstream,
			});
		case 'pull-and-push':
			return intl.formatMessage(sourceControlMessages.syncPullAndPush, {
				ahead: tooltip.ahead,
				behind: tooltip.behind,
				upstream: tooltip.upstream,
			});
		case 'push':
			return intl.formatMessage(sourceControlMessages.syncPush, {
				count: tooltip.ahead,
				upstream: tooltip.upstream,
			});
		case 'synchronize':
			return intl.formatMessage(sourceControlMessages.syncSynchronize);
		case 'syncing':
			return intl.formatMessage(sourceControlMessages.syncSyncing);
	}
}
