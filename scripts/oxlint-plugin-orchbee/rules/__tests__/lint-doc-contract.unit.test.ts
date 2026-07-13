import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import orchbeePlugin from "../../index.js";

interface OxlintConfig {
  readonly rules: Readonly<Record<string, unknown>>;
  readonly overrides?: readonly {
    readonly rules?: Readonly<Record<string, unknown>>;
  }[];
}

const REPOSITORY_ROOT = resolve(import.meta.dirname, "../../../..");
const DOCS_ROOT = join(REPOSITORY_ROOT, "docs");
const RULES_ROOT = resolve(import.meta.dirname, "..");

function collectFiles(directory: string, extension: string): readonly string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return collectFiles(path, extension);
    return entry.name.endsWith(extension) ? [path] : [];
  });
}

function markdownAnchor(heading: string): string {
  return heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}\s_-]/gu, "")
    .replace(/\s+/g, "-");
}

function documentAnchors(documentPath: string): ReadonlySet<string> {
  const markdown = readFileSync(documentPath, "utf8");
  const headings = markdown.matchAll(/^#{1,6}\s+(.+)$/gm);
  return new Set(Array.from(headings, (match) => markdownAnchor(match[1] ?? "")));
}

function lintConfig(): OxlintConfig {
  const configPath = join(REPOSITORY_ROOT, ".oxlintrc.json");
  return JSON.parse(readFileSync(configPath, "utf8")) as OxlintConfig;
}

function configuredRules(): Readonly<Record<string, unknown>> {
  return lintConfig().rules;
}

function explicitlyEnabledRules(): readonly string[] {
  const config = lintConfig();
  const ruleSets = [
    config.rules,
    ...(config.overrides ?? []).map((override) => override.rules ?? {}),
  ];
  return [
    ...new Set(
      ruleSets.flatMap((rules) =>
        Object.entries(rules)
          .filter(([, setting]) => setting !== "off")
          .map(([rule]) => rule),
      ),
    ),
  ];
}

function diagnosticAuthorities(): Readonly<Record<string, string>> {
  return Object.fromEntries(
    Object.keys(orchbeePlugin.rules).map((rule) => {
      const ruleFile = join(RULES_ROOT, `${rule}.js`);
      const source = readFileSync(ruleFile, "utf8");
      const links = Array.from(
        source.matchAll(/See (docs\/[A-Za-z0-9_./-]+\.md)#([a-z0-9-]+)/g),
        (match) => `${match[1] ?? ""}#${match[2] ?? ""}`,
      );
      const authorities = [...new Set(links)];
      const [authority] = authorities;

      if (authority === undefined || authorities.length !== 1) {
        throw new Error(`${ruleFile} must reference exactly one authoritative docs target`);
      }

      return [`orchbee/${rule}`, authority];
    }),
  );
}

describe("lint-doc contract", () => {
  it("keeps every custom diagnostic docs anchor valid", () => {
    const authorities = Object.entries(diagnosticAuthorities());

    expect(authorities).not.toEqual([]);
    for (const [rule, authority] of authorities) {
      const [document = "", anchor = ""] = authority.split("#");
      const documentPath = join(REPOSITORY_ROOT, document);
      expect({
        link: `${rule}: ${document}`,
        valid: existsSync(documentPath),
      }).toEqual({ link: `${rule}: ${document}`, valid: true });
      expect({
        link: `${rule}: ${authority}`,
        valid: documentAnchors(documentPath).has(anchor),
      }).toEqual({ link: `${rule}: ${authority}`, valid: true });
    }
  });

  it("documents every explicitly enabled rule", () => {
    const documentation = collectFiles(DOCS_ROOT, ".md")
      .map((file) => readFileSync(file, "utf8"))
      .join("\n");
    const undocumentedRules = explicitlyEnabledRules().filter(
      (rule) => !documentation.includes(`\`${rule}\``),
    );

    expect(undocumentedRules).toEqual([]);
  });

  it("keeps custom plugin exports, config, and ownership docs synchronized", () => {
    const configuredCustomRules = Object.entries(configuredRules())
      .filter(([rule, setting]) => rule.startsWith("orchbee/") && setting !== "off")
      .map(([rule]) => rule)
      .toSorted();
    const exportedCustomRules = Object.keys(orchbeePlugin.rules)
      .map((rule) => `orchbee/${rule}`)
      .toSorted();
    const lintingDoc = readFileSync(join(DOCS_ROOT, "LINTING.md"), "utf8");
    const documentedCustomRules = Array.from(
      lintingDoc.matchAll(/^\|\s*`(orchbee\/[^`]+)`\s*\|/gm),
      (match) => match[1] ?? "",
    ).toSorted();
    const documentedAuthorities = Object.fromEntries(
      Array.from(
        lintingDoc.matchAll(
          /^\|\s*`(orchbee\/[^`]+)`\s*\|\s*`(docs\/[A-Za-z0-9_./-]+\.md#[a-z0-9-]+)`\s*\|/gm,
        ),
        (match) => [match[1] ?? "", match[2] ?? ""],
      ),
    );

    expect(configuredCustomRules).toEqual(exportedCustomRules);
    expect(configuredCustomRules).toEqual(documentedCustomRules);
    expect(diagnosticAuthorities()).toEqual(documentedAuthorities);
  });
});
