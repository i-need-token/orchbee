import { describe, expect, it } from 'vitest';

import { ORCHBEE_LOCALES } from '../../locale';
import { getOrchbeeMessages } from '../getOrchbeeMessages';

function collectVariables(message: string): ReadonlySet<string> {
	const variables = new Set<string>();
	for (const match of message.matchAll(/\{([A-Za-z][A-Za-z0-9_]*)\s*(?:,|\})/gu)) {
		const variable = match[1];
		if (variable !== undefined) variables.add(variable);
	}
	return variables;
}

describe('getOrchbeeMessages', () => {
	it('keeps locale IDs and variables aligned with the English source catalog', () => {
		const sourceMessages = getOrchbeeMessages('en-US');
		const sourceIds = new Set(Object.keys(sourceMessages));

		for (const locale of ORCHBEE_LOCALES) {
			const messages = getOrchbeeMessages(locale);
			expect(new Set(Object.keys(messages))).toEqual(sourceIds);

			for (const [messageId, sourceMessage] of Object.entries(sourceMessages)) {
				const message = messages[messageId];
				if (message === undefined) throw new Error(`Missing ${locale}: ${messageId}`);
				expect(collectVariables(message)).toEqual(collectVariables(sourceMessage));
			}
		}
	});

	it('ships localized Workbench interface labels', () => {
		expect(getOrchbeeMessages('en-US')['workbench.sidebar.items.newTerminal']).toBe('New Terminal');
		expect(getOrchbeeMessages('zh-CN')['workbench.sidebar.items.newTerminal']).toBe('新建终端');
		expect(getOrchbeeMessages('en-US')['workbench.sidebar.items.search']).toBe('Search');
		expect(getOrchbeeMessages('zh-CN')['workbench.sidebar.items.search']).toBe('搜索');
		expect(getOrchbeeMessages('en-US')['workbench.sidebar.items.settings']).toBe('Settings');
		expect(getOrchbeeMessages('zh-CN')['workbench.sidebar.items.settings']).toBe('设置');
		expect(getOrchbeeMessages('en-US')['workbench.sidebar.sections.project']).toBe('Project');
		expect(getOrchbeeMessages('zh-CN')['workbench.sidebar.sections.project']).toBe('项目');
		expect(getOrchbeeMessages('en-US')['workbench.sidebar.actions.hide']).toBe('Hide Sidebar');
		expect(getOrchbeeMessages('zh-CN')['workbench.sidebar.actions.hide']).toBe('隐藏边栏');
		expect(getOrchbeeMessages('en-US')['workbench.tabs.label']).toBe('Workbench tabs');
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.label']).toBe('工作区标签页');
		expect(getOrchbeeMessages('en-US')['workbench.tabs.actions.add']).toBe('New Tab');
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.actions.add']).toBe('新建标签页');
		expect(getOrchbeeMessages('en-US')['workbench.tabs.actions.close']).toBe('Close {tabLabel}');
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.actions.close']).toBe('关闭“{tabLabel}”');
		expect(getOrchbeeMessages('en-US')['workbench.tabs.contextMenu.actions.closeOthers']).toBe(
			'Close Other Tabs',
		);
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.contextMenu.actions.closeOthers']).toBe(
			'关闭其他标签页',
		);
		expect(getOrchbeeMessages('en-US')['workbench.tabs.contextMenu.actions.rename']).toBe(
			'Rename Tab…',
		);
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.contextMenu.actions.rename']).toBe(
			'重命名标签页…',
		);
		expect(getOrchbeeMessages('en-US')['workbench.tabs.contextMenu.tabColor']).toBe('Tab Color');
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.contextMenu.tabColor']).toBe('标签页颜色');
		expect(getOrchbeeMessages('en-US')['workbench.tabs.contextMenu.actions.addToNewGroup']).toBe(
			'Add Tab to New Group',
		);
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.contextMenu.actions.addToNewGroup']).toBe(
			'添加标签页到新组',
		);
		expect(getOrchbeeMessages('en-US')['workbench.tabs.groups.actions.ungroup']).toBe(
			'Ungroup Tabs',
		);
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.groups.actions.ungroup']).toBe('取消分组');
		expect(getOrchbeeMessages('en-US')['workbench.tabs.groups.actions.rename']).toBe(
			'Rename Group…',
		);
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.groups.actions.rename']).toBe(
			'重命名标签组…',
		);
		expect(getOrchbeeMessages('en-US')['workbench.tabs.rename.description']).toBe(
			'Enter a new name for “{tabLabel}”.',
		);
		expect(getOrchbeeMessages('zh-CN')['workbench.tabs.rename.description']).toBe(
			'为“{tabLabel}”输入新名称。',
		);
	});
});
