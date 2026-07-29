import { componentDirectory } from './rules/component-directory.js';
import { filenameNamingConvention } from './rules/filename-naming-convention.js';
import { moduleBoundaries } from './rules/module-boundaries.js';
import { noCrossWorkspaceSourceImports } from './rules/no-cross-workspace-source-imports.js';
import { noInlineSuppressions } from './rules/no-inline-suppressions.js';
import { noReactHooksInTsx } from './rules/no-react-hooks-in-tsx.js';
import { noReactUseCallback } from './rules/no-react-use-callback.js';
import { oneFunctionPerTsx } from './rules/one-function-per-tsx.js';
import { oneHookPerFile } from './rules/one-hook-per-file.js';
import { requireUnitTest } from './rules/require-unit-test.js';
import { zustandScope } from './rules/zustand-scope.js';

/** @type {import('eslint').ESLint.Plugin} */
export default {
	rules: {
		'component-directory': componentDirectory,
		'filename-naming-convention': filenameNamingConvention,
		'module-boundaries': moduleBoundaries,
		'no-cross-workspace-source-imports': noCrossWorkspaceSourceImports,
		'no-inline-suppressions': noInlineSuppressions,
		'no-react-hooks-in-tsx': noReactHooksInTsx,
		'no-react-use-callback': noReactUseCallback,
		'one-function-per-tsx': oneFunctionPerTsx,
		'one-hook-per-file': oneHookPerFile,
		'require-unit-test': requireUnitTest,
		'zustand-scope': zustandScope,
	},
};
