import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { TabModel } from '@orchbee/tab-strip';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { useWorkbenchPanelWorkspace } from '../useWorkbenchPanelWorkspace';

const tabModel = new TabModel({
	activeTabId: 'tab-a',
	tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
});

function HookProjection() {
	const view = useWorkbenchPanelWorkspace({
		tabModel,
	});
	return createElement(
		'div',
		null,
		`${view.activeTab?.tabId}:${view.messages.newChildTerminal}:${view.state.tabs.length}:${
			'layoutKey' in view.panelDrag
		}`,
	);
}

describe('useWorkbenchPanelWorkspace', () => {
	it('projects the active tab and localized panel actions on the first render', () => {
		const markup = renderToStaticMarkup(
			createElement(OrchbeeI18nProvider, { locale: 'zh-CN' }, createElement(HookProjection)),
		);

		expect(markup).toContain('tab-a:新建子终端:1:false');
	});
});
