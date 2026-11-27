/**
 * Installs the pinned package's zh-CN message table before any Monaco module evaluates, because
 * Monaco reads its messages by index from that page-wide table while its modules load.
 */
import 'monaco-editor/nls/lang/zh-cn';
