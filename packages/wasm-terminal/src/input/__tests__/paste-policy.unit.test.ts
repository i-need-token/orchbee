import { describe, expect, it } from "vitest";
import { encodePaste, inspectPaste } from "../paste-policy.js";

describe("paste policy", () => {
  it("replaces terminal control bytes", () => {
    const inspection = inspectPaste("hel\x1blo\0\x03world", { bracketed: true });
    expect(inspection.sanitized).toBe("hel lo  world");
    expect(inspection.replacedControlBytes).toBe(3);
  });

  it("converts newlines only for non-bracketed paste", () => {
    const plain = inspectPaste("first\nsecond", {
      bracketed: false,
      allowUnsafe: true,
    });
    const bracketed = inspectPaste("first\nsecond", { bracketed: true });
    expect(encodePaste(plain)).toBe("first\rsecond");
    expect(encodePaste(bracketed)).toBe("\x1b[200~first\nsecond\x1b[201~");
  });

  it("requires confirmation for multiline non-bracketed paste", () => {
    expect(inspectPaste("first\nsecond", { bracketed: false }).classification).toBe("confirm");
    expect(inspectPaste("first\nsecond", { bracketed: true }).classification).toBe("safe");
  });

  it("never trusts an embedded bracketed-paste terminator", () => {
    const inspection = inspectPaste(`safe\x1b[201~unsafe`, { bracketed: true });
    expect(inspection.embeddedBracketTerminator).toBe(true);
    expect(inspection.classification).toBe("confirm");
    expect(inspection.sanitized).toBe("safe [201~unsafe");
  });

  it("supports disabling protection without disabling sanitization", () => {
    const inspection = inspectPaste("first\n\x1bsecond", {
      bracketed: false,
      protection: false,
    });
    expect(inspection.classification).toBe("safe");
    expect(inspection.sanitized).toBe("first\n second");
  });
});
