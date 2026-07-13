import { filenameNamingConvention } from "./rules/filename-naming-convention.js";
import { noCrossWorkspaceSourceImports } from "./rules/no-cross-workspace-source-imports.js";
import { noInlineSuppressions } from "./rules/no-inline-suppressions.js";
import { requireUnitTest } from "./rules/require-unit-test.js";

/** @type {import('eslint').ESLint.Plugin} */
export default {
  rules: {
    "filename-naming-convention": filenameNamingConvention,
    "no-cross-workspace-source-imports": noCrossWorkspaceSourceImports,
    "no-inline-suppressions": noInlineSuppressions,
    "require-unit-test": requireUnitTest,
  },
};
