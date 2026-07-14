import crypto from "node:crypto";
import childProcess from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const repositoryRoot = process.cwd();
const ghosttyRoot = path.resolve(process.env.GHOSTTY_SOURCE ?? "../ghostty");
const sourcePath = path.join(ghosttyRoot, "src/font/nerd_font_attributes.zig");
const outputPath = path.join(
  repositoryRoot,
  "packages/terminal/src/nerd-font-constraints.generated.ts",
);
const source = fs.readFileSync(sourcePath, "utf8");
const sourceHash = crypto.createHash("sha256").update(source).digest("hex");
const generated = formatGenerated(generate(source, sourceHash));

if (process.argv.includes("--check")) {
  const current = fs.readFileSync(outputPath, "utf8");
  if (current !== generated) {
    process.stderr.write(`${path.relative(repositoryRoot, outputPath)} is stale\n`);
    process.exitCode = 1;
  }
} else {
  fs.writeFileSync(outputPath, generated);
}

function generate(input, hash) {
  const rulePattern =
    /((?:\s*0x[0-9a-f]+(?:\.\.\.0x[0-9a-f]+)?\s*,\n)+)\s*=> \.\{\n([\s\S]*?)\n\s*\},/gu;
  const rules = [];
  for (const match of input.matchAll(rulePattern)) {
    const selectors = [...match[1].matchAll(/0x([0-9a-f]+)(?:\.\.\.0x([0-9a-f]+))?/gu)].map(
      (selector) => [
        Number.parseInt(selector[1], 16),
        Number.parseInt(selector[2] ?? selector[1], 16),
      ],
    );
    const constraint = {};
    for (const line of match[2].split(/\r?\n/u)) {
      if (line.trim().length === 0) continue;
      const field = line.match(/^\s*\.([a-z_]+) = ([^,]+),$/u);
      if (!field) throw new Error(`Unsupported Ghostty constraint field: ${line}`);
      constraint[toCamelCase(field[1])] = parseValue(field[2]);
    }
    rules.push({ selectors, constraint });
  }
  if (rules.length === 0) throw new Error("No Ghostty Nerd Font constraints were parsed");

  const lines = [
    "/**",
    " * Generated from Ghostty src/font/nerd_font_attributes.zig.",
    " * Regenerate with `pnpm generate:ghostty-panel-data`; do not edit by hand.",
    ` * Source SHA-256: ${hash}`,
    " */",
    "",
    "export interface GeneratedGlyphConstraint {",
    '  readonly size?: "none" | "fit" | "cover" | "fit_cover1" | "stretch";',
    '  readonly height?: "cell" | "icon";',
    '  readonly alignVertical?: "none" | "start" | "end" | "center" | "center1";',
    '  readonly alignHorizontal?: "none" | "start" | "end" | "center" | "center1";',
    "  readonly padTop?: number;",
    "  readonly padLeft?: number;",
    "  readonly padRight?: number;",
    "  readonly padBottom?: number;",
    "  readonly relativeWidth?: number;",
    "  readonly relativeHeight?: number;",
    "  readonly relativeX?: number;",
    "  readonly relativeY?: number;",
    "  readonly maxXyRatio?: number;",
    "  readonly maxConstraintWidth?: number;",
    "}",
    "",
    "export interface NerdFontConstraintRule {",
    "  readonly ranges: readonly (readonly [start: number, end: number])[];",
    "  readonly constraint: GeneratedGlyphConstraint;",
    "}",
    "",
    "export const NERD_FONT_CONSTRAINT_RULES: readonly NerdFontConstraintRule[] = [",
  ];
  for (const rule of rules) {
    const ranges = rule.selectors
      .map(([start, end]) => `[${formatCodepoint(start)}, ${formatCodepoint(end)}]`)
      .join(", ");
    lines.push(`  { ranges: [${ranges}], constraint: ${JSON.stringify(rule.constraint)} },`);
  }
  lines.push("];", "");
  return lines.join("\n");
}

function parseValue(value) {
  if (value.startsWith(".")) return value.slice(1);
  const number = Number(value);
  if (!Number.isFinite(number)) throw new Error(`Unsupported Ghostty constraint value: ${value}`);
  return number;
}

function toCamelCase(value) {
  return value.replace(/_([a-z])/gu, (_match, letter) => letter.toUpperCase());
}

function formatCodepoint(value) {
  return `0x${value.toString(16)}`;
}

function formatGenerated(input) {
  const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "orchbee-nerd-font-"));
  const temporaryFile = path.join(temporaryDirectory, "constraints.ts");
  try {
    fs.writeFileSync(temporaryFile, input);
    const result = childProcess.spawnSync("pnpm", ["exec", "oxfmt", temporaryFile], {
      cwd: repositoryRoot,
      encoding: "utf8",
    });
    if (result.status !== 0) {
      throw new Error(result.stderr || result.stdout || "oxfmt failed");
    }
    return fs.readFileSync(temporaryFile, "utf8");
  } finally {
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}
