import { describe, expect, it } from "vitest";
import { filenameNamingConvention } from "../filename-naming-convention.js";
import { noCrossWorkspaceSourceImports } from "../no-cross-workspace-source-imports.js";
import { noInlineSuppressions } from "../no-inline-suppressions.js";
import { requireUnitTest } from "../require-unit-test.js";

interface LintReport {
  readonly message: string;
}

interface CommentNode {
  readonly value: string;
  readonly loc: {
    readonly start: { readonly line: number; readonly column: number };
  };
}

function createContext(filename: string, comments: readonly CommentNode[] = []) {
  const reports: LintReport[] = [];
  return {
    reports,
    context: {
      filename,
      sourceCode: {
        getAllComments: () => comments,
      },
      report: (report: LintReport): void => {
        reports.push(report);
      },
    },
  };
}

const LOCATION = {
  start: { line: 1, column: 0 },
  end: { line: 1, column: 1 },
};

describe("filename-naming-convention", () => {
  it("accepts sibling TSX unit tests with kebab-case paths", () => {
    const { context, reports } = createContext(
      "/workspace/packages/ui/src/components/__tests__/agent-panel.unit.test.tsx",
    );
    filenameNamingConvention.create(context).Program({ loc: LOCATION });
    expect(reports).toEqual([]);
  });

  it("reports the extension-preserving TSX unit-test suffix", () => {
    const { context, reports } = createContext(
      "/workspace/packages/ui/src/components/__tests__/agent-panel.test.tsx",
    );
    filenameNamingConvention.create(context).Program({ loc: LOCATION });
    expect(reports[0]?.message).toContain("<source-name>.unit.test.tsx");
  });
});

describe("no-cross-workspace-source-imports", () => {
  it("rejects re-exports from another package source tree", () => {
    const { context, reports } = createContext("/workspace/apps/desktop/src/terminal.ts");
    noCrossWorkspaceSourceImports.create(context).ExportNamedDeclaration({
      source: {
        value: "../../../packages/terminal/src/terminal.js",
        loc: LOCATION,
      },
    });
    expect(reports[0]?.message).toContain('Import "@orchbee/terminal"');
  });

  it("accepts package public exports", () => {
    const { context, reports } = createContext("/workspace/apps/desktop/src/terminal.ts");
    noCrossWorkspaceSourceImports.create(context).ImportDeclaration({
      source: { value: "@orchbee/terminal", loc: LOCATION },
    });
    expect(reports).toEqual([]);
  });
});

describe("no-inline-suppressions", () => {
  it("rejects lint suppressions but leaves TypeScript directives to ban-ts-comment", () => {
    const comments = [
      { value: " eslint-disable-next-line no-console", loc: LOCATION },
      { value: " @ts-expect-error -- verifies invalid input", loc: LOCATION },
    ];
    const { context, reports } = createContext("/workspace/example.unit.test.ts", comments);
    noInlineSuppressions.create(context).Program();
    expect(reports).toHaveLength(1);
    expect(reports[0]?.message).toContain("eslint-disable");
  });
});

describe("require-unit-test", () => {
  it("requires a sibling TSX test for a top-level arrow function", () => {
    const { context, reports } = createContext(
      "/workspace/packages/ui/src/components/agent-panel.tsx",
    );
    const program = { type: "Program" };
    const exportDeclaration = { type: "ExportNamedDeclaration", parent: program };
    const variableDeclaration = { type: "VariableDeclaration", parent: exportDeclaration };
    requireUnitTest.create(context).VariableDeclarator({
      parent: variableDeclaration,
      init: { type: "ArrowFunctionExpression" },
      id: { loc: LOCATION },
    });
    expect(reports[0]?.message).toContain("agent-panel.unit.test.tsx");
  });

  it("does not require tests for type-only source files", () => {
    const { context, reports } = createContext("/workspace/packages/core/src/types.ts");
    const visitors = requireUnitTest.create(context);
    expect(visitors).not.toHaveProperty("Program");
    expect(reports).toEqual([]);
  });
});
