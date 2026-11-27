/**
 * The Monaco editor as Orchbee composes it: the core API, the editor features a fresh VS Code
 * offers without language services, every Monarch basic language the package lists, and the editor
 * worker. Apart from `monaco-messages.ts`, which must run first, this is the only module that
 * imports `monaco-editor` at run time, and only `code-editor-runtime.ts`, which `loadCodeEditor`
 * loads, imports it, so the main bundle never contains the editor.
 */
import 'monaco-editor/features/anchorSelect/register';
import 'monaco-editor/features/bracketMatching/register';
import 'monaco-editor/features/caretOperations/register';
import 'monaco-editor/features/clipboard/register';
import 'monaco-editor/features/codicon/register';
import 'monaco-editor/features/comment/register';
import 'monaco-editor/features/cursorUndo/register';
import 'monaco-editor/features/diffEditor/register';
import 'monaco-editor/features/dnd/register';
import 'monaco-editor/features/find/register';
import 'monaco-editor/features/folding/register';
import 'monaco-editor/features/fontZoom/register';
import 'monaco-editor/features/hover/register';
import 'monaco-editor/features/indentation/register';
import 'monaco-editor/features/inPlaceReplace/register';
import 'monaco-editor/features/insertFinalNewLine/register';
import 'monaco-editor/features/lineSelection/register';
import 'monaco-editor/features/linesOperations/register';
import 'monaco-editor/features/links/register';
import 'monaco-editor/features/longLinesHelper/register';
import 'monaco-editor/features/multicursor/register';
import 'monaco-editor/features/placeholderText/register';
import 'monaco-editor/features/readOnlyMessage/register';
import 'monaco-editor/features/sectionHeaders/register';
import 'monaco-editor/features/smartSelect/register';
import 'monaco-editor/features/stickyScroll/register';
import 'monaco-editor/features/toggleTabFocusMode/register';
import 'monaco-editor/features/unicodeHighlighter/register';
import 'monaco-editor/features/wordHighlighter/register';
import 'monaco-editor/features/wordOperations/register';
import 'monaco-editor/features/wordPartOperations/register';
import 'monaco-editor/languages/definitions/abap/register';
import 'monaco-editor/languages/definitions/apex/register';
import 'monaco-editor/languages/definitions/azcli/register';
import 'monaco-editor/languages/definitions/bat/register';
import 'monaco-editor/languages/definitions/bicep/register';
import 'monaco-editor/languages/definitions/cameligo/register';
import 'monaco-editor/languages/definitions/clojure/register';
import 'monaco-editor/languages/definitions/coffee/register';
import 'monaco-editor/languages/definitions/cpp/register';
import 'monaco-editor/languages/definitions/csharp/register';
import 'monaco-editor/languages/definitions/csp/register';
import 'monaco-editor/languages/definitions/css/register';
import 'monaco-editor/languages/definitions/cypher/register';
import 'monaco-editor/languages/definitions/dart/register';
import 'monaco-editor/languages/definitions/dockerfile/register';
import 'monaco-editor/languages/definitions/ecl/register';
import 'monaco-editor/languages/definitions/elixir/register';
import 'monaco-editor/languages/definitions/flow9/register';
import 'monaco-editor/languages/definitions/freemarker2/register';
import 'monaco-editor/languages/definitions/fsharp/register';
import 'monaco-editor/languages/definitions/go/register';
import 'monaco-editor/languages/definitions/graphql/register';
import 'monaco-editor/languages/definitions/handlebars/register';
import 'monaco-editor/languages/definitions/hcl/register';
import 'monaco-editor/languages/definitions/html/register';
import 'monaco-editor/languages/definitions/ini/register';
import 'monaco-editor/languages/definitions/java/register';
import 'monaco-editor/languages/definitions/javascript/register';
import 'monaco-editor/languages/definitions/julia/register';
import 'monaco-editor/languages/definitions/kotlin/register';
import 'monaco-editor/languages/definitions/less/register';
import 'monaco-editor/languages/definitions/lexon/register';
import 'monaco-editor/languages/definitions/liquid/register';
import 'monaco-editor/languages/definitions/lua/register';
import 'monaco-editor/languages/definitions/m3/register';
import 'monaco-editor/languages/definitions/markdown/register';
import 'monaco-editor/languages/definitions/mdx/register';
import 'monaco-editor/languages/definitions/mips/register';
import 'monaco-editor/languages/definitions/msdax/register';
import 'monaco-editor/languages/definitions/mysql/register';
import 'monaco-editor/languages/definitions/objective-c/register';
import 'monaco-editor/languages/definitions/pascal/register';
import 'monaco-editor/languages/definitions/pascaligo/register';
import 'monaco-editor/languages/definitions/perl/register';
import 'monaco-editor/languages/definitions/pgsql/register';
import 'monaco-editor/languages/definitions/php/register';
import 'monaco-editor/languages/definitions/pla/register';
import 'monaco-editor/languages/definitions/postiats/register';
import 'monaco-editor/languages/definitions/powerquery/register';
import 'monaco-editor/languages/definitions/powershell/register';
import 'monaco-editor/languages/definitions/protobuf/register';
import 'monaco-editor/languages/definitions/pug/register';
import 'monaco-editor/languages/definitions/python/register';
import 'monaco-editor/languages/definitions/qsharp/register';
import 'monaco-editor/languages/definitions/r/register';
import 'monaco-editor/languages/definitions/razor/register';
import 'monaco-editor/languages/definitions/redis/register';
import 'monaco-editor/languages/definitions/redshift/register';
import 'monaco-editor/languages/definitions/restructuredtext/register';
import 'monaco-editor/languages/definitions/ruby/register';
import 'monaco-editor/languages/definitions/rust/register';
import 'monaco-editor/languages/definitions/sb/register';
import 'monaco-editor/languages/definitions/scala/register';
import 'monaco-editor/languages/definitions/scheme/register';
import 'monaco-editor/languages/definitions/scss/register';
import 'monaco-editor/languages/definitions/shell/register';
import 'monaco-editor/languages/definitions/solidity/register';
import 'monaco-editor/languages/definitions/sophia/register';
import 'monaco-editor/languages/definitions/sparql/register';
import 'monaco-editor/languages/definitions/sql/register';
import 'monaco-editor/languages/definitions/st/register';
import 'monaco-editor/languages/definitions/swift/register';
import 'monaco-editor/languages/definitions/systemverilog/register';
import 'monaco-editor/languages/definitions/tcl/register';
import 'monaco-editor/languages/definitions/twig/register';
import 'monaco-editor/languages/definitions/typescript/register';
import 'monaco-editor/languages/definitions/typespec/register';
import 'monaco-editor/languages/definitions/vb/register';
import 'monaco-editor/languages/definitions/wgsl/register';
import 'monaco-editor/languages/definitions/xml/register';
import 'monaco-editor/languages/definitions/yaml/register';

export { editor, languages } from 'monaco-editor/editor/editor.api';

const EDITOR_WORKER_LABEL = 'editorWorkerService';

/**
 * Starts the editor worker as a same-origin module worker, which `script-src 'self'` admits.
 * Without `getWorker`, Monaco would start it from a `blob:` URL, which the policy refuses. No
 * language worker is bundled, so any other label is a composition error.
 */
function createEditorWorker(_workerId: string, label: string): Worker {
	if (label !== EDITOR_WORKER_LABEL) {
		throw new Error(`The code editor bundles no worker for ${label}.`);
	}
	return new Worker(new URL('monaco-editor/editor/editor.worker', import.meta.url), {
		name: EDITOR_WORKER_LABEL,
		type: 'module',
	});
}

globalThis.MonacoEnvironment = { getWorker: createEditorWorker };
