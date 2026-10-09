import { componentDirectory } from './rules/component-directory.js';
import { filenameNamingConvention } from './rules/filename-naming-convention.js';
import { moduleBoundaries } from './rules/module-boundaries.js';
import { noCrossWorkspaceSourceImports } from './rules/no-cross-workspace-source-imports.js';
import { noInlineSuppressions } from './rules/no-inline-suppressions.js';
import { noReactUseCallback } from './rules/no-react-use-callback.js';
import { componentExportBoundary } from './rules/component-export-boundary.js';
import { hookExportBoundary } from './rules/hook-export-boundary.js';
import { zustandScope } from './rules/zustand-scope.js';

/** @type {import('eslint').ESLint.Plugin} */
export default {
	rules: {
		'component-directory': componentDirectory,
		'filename-naming-convention': filenameNamingConvention,
		'module-boundaries': moduleBoundaries,
		'no-cross-workspace-source-imports': noCrossWorkspaceSourceImports,
		'no-inline-suppressions': noInlineSuppressions,
		'no-react-use-callback': noReactUseCallback,
		'component-export-boundary': componentExportBoundary,
		'hook-export-boundary': hookExportBoundary,
		'zustand-scope': zustandScope,
	},
};
