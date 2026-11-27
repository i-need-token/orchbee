import type { MonacoMessageIndex } from '../en-US';

/**
 * Orchbee's Chinese for the Monaco messages that the pinned `monaco-editor`'s zh-CN table leaves in
 * English or words differently from its neighbors, in the VS Code Chinese language pack's terms.
 * Each replaces its index in that table before any Monaco module reads it.
 */
export const zhCNMonacoMessageOverrides = {
	145: '折叠未更改的行',
	146: '显示 {0} 个隐藏的行',
	149: '折叠未更改的行',
	150: '转到 {0}',
	153: '代码已移动至行 {0}-{1}',
	1014: '转到匹配项...',
	1198: '由于性能原因，长行的呈现已暂停。可通过 `editor.stopRenderingLineAfter` 配置此设置。[不再显示](command:{0})',
	1364: '添加的光标: {0}',
	1529: '聚焦编辑器粘滞滚动',
	1530: '选择下一个编辑器粘滞滚动行',
	1531: '选择上一个粘滞滚动行',
	1532: '转到聚焦的粘滞滚动行',
	1627: '禁用注释中的突出显示',
	1635: '禁用注释中字符的突出显示',
} as const satisfies Readonly<Record<MonacoMessageIndex, string>>;
