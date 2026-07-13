import { dirname, resolve } from "node:path";
import { startLoc } from "../utils.js";

const PACKAGE_SOURCE_IMPORT = /^@orchbee\/([^/]+)\/src(?:\/|$)/;
const ROOT_SOURCE_IMPORT = /^(apps|packages)\/([^/]+)\/src(?:\/|$)/;

function normalize(filePath) {
  return filePath.split("\\").join("/");
}

function findWorkspaceUnit(filePath) {
  const normalized = normalize(filePath);
  const match = normalized.match(/\/(apps|packages)\/([^/]+)(?:\/|$)/);
  if (!match || match.index === undefined) return null;
  const [matchedPath, kind, name] = match;
  if (!kind || !name) return null;
  const root = normalized.slice(0, match.index + matchedPath.length).replace(/\/$/, "");
  return { kind, name, root };
}

function publicPackageName(unit) {
  return unit.kind === "packages" ? `@orchbee/${unit.name}` : null;
}

function report(context, node, source, targetUnit) {
  const packageName = publicPackageName(targetUnit);
  const action = packageName
    ? `Import "${packageName}" through its package exports instead of "${source}"`
    : `Move shared code into a package instead of importing app source "${source}"`;
  context.report({
    loc: startLoc(node.source ?? node),
    message: `${action}. See docs/ARCHITECTURE.md#package-boundaries.`,
  });
}

/** @type {import('eslint').Rule.RuleModule} */
export const noCrossWorkspaceSourceImports = {
  meta: {
    type: "problem",
    docs: {
      description: "Disallow imports that cross directly into another workspace source tree.",
    },
    schema: [],
  },
  create(context) {
    const currentUnit = findWorkspaceUnit(context.filename);

    function checkSource(node) {
      const source = node.source?.value;
      if (typeof source !== "string") return;

      const packageMatch = source.match(PACKAGE_SOURCE_IMPORT);
      if (packageMatch?.[1]) {
        report(context, node, source, { kind: "packages", name: packageMatch[1], root: "" });
        return;
      }

      const rootMatch = source.match(ROOT_SOURCE_IMPORT);
      if (rootMatch?.[1] && rootMatch[2]) {
        report(context, node, source, { kind: rootMatch[1], name: rootMatch[2], root: "" });
        return;
      }

      if (!currentUnit || !source.startsWith(".")) return;
      const targetPath = normalize(resolve(dirname(context.filename), source));
      const targetUnit = findWorkspaceUnit(targetPath);
      if (!targetUnit || targetUnit.root === currentUnit.root) return;
      if (
        targetPath !== `${targetUnit.root}/src` &&
        !targetPath.startsWith(`${targetUnit.root}/src/`)
      ) {
        return;
      }
      report(context, node, source, targetUnit);
    }

    return {
      ImportDeclaration: checkSource,
      ImportExpression: checkSource,
      ExportAllDeclaration: checkSource,
      ExportNamedDeclaration: checkSource,
    };
  },
};
