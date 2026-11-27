/**
 * Installs the pinned package's zh-CN message table, with Orchbee's overrides written into it,
 * before any Monaco module evaluates: Monaco reads its messages by index from that page-wide table
 * while its modules load and whenever a widget renders.
 */
import 'monaco-editor/nls/lang/zh-cn';

import { zhCNMonacoMessageOverrides } from '@orchbee/i18n';

const table: unknown = Reflect.get(globalThis, '_VSCODE_NLS_MESSAGES');
if (!Array.isArray(table)) {
	throw new TypeError('The pinned zh-CN Monaco message table did not install itself.');
}
for (const [index, message] of Object.entries(zhCNMonacoMessageOverrides)) {
	table[Number(index)] = message;
}
