import type { OrchbeeLocale } from '@orchbee/i18n';

import type * as MonacoEntry from './monaco-entry';

type CodeEditorRuntime = typeof MonacoEntry;

/**
 * Monaco's services, worker environment and message table are page-wide, so the first load fixes
 * the editor's language until the page reloads; a later language change reaches Monaco only then.
 */
let runtime: Promise<CodeEditorRuntime> | undefined;

/** Loads the code editor chunk once per page; a failed load leaves the next call free to retry. */
export function loadCodeEditor(locale: OrchbeeLocale): Promise<CodeEditorRuntime> {
	if (runtime === undefined) {
		const loading = importCodeEditorRuntime(locale);
		runtime = loading;
		loading.catch(() => {
			if (runtime === loading) runtime = undefined;
		});
	}
	return runtime;
}

async function importCodeEditorRuntime(locale: OrchbeeLocale): Promise<CodeEditorRuntime> {
	// Monaco reads its messages while its modules evaluate, so the table must be in place first.
	if (locale === 'zh-CN') await import('./monaco-messages');
	return import('./monaco-entry');
}
