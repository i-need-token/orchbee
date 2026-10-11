import { describe, expect, it } from 'vitest';
import { renderPage } from '../prerender';

describe('localized static website', () => {
	it.each(['en-US', 'zh-CN'] as const)(
		'renders crawlable %s content without a browser runtime',
		(locale) => {
			const page = renderPage(locale);
			expect(page.markup.match(/<h1>/gu)).toHaveLength(1);
			expect(page.markup).toContain(locale === 'zh-CN' ? '一个终端。' : 'One terminal.');
			expect(page.markup).toContain(
				locale === 'zh-CN' ? '暂未提供预编译下载包' : 'Pre-built downloads are not available',
			);
			expect(page.markup).toContain('<summary>');
			expect(page.markup).toContain(locale === 'zh-CN' ? '外观' : 'Appearance');
			expect(page.markup).not.toContain('<iframe');
			expect(page.markup).not.toContain('<form');
		},
	);
	it('gives both languages stable canonical URLs and localized search metadata', () => {
		const english = renderPage('en-US');
		const chinese = renderPage('zh-CN');
		expect(english.canonical).toBe('https://orchbee.com/');
		expect(chinese.canonical).toBe('https://orchbee.com/zh-CN/');
		expect(english.alternate).toBe('/zh-CN/');
		expect(chinese.alternate).toBe('/');
		expect(english.title).toContain('AI agents');
		expect(chinese.title).toContain('AI 智能体');
		expect(english.description).not.toEqual(chinese.description);
	});
});
