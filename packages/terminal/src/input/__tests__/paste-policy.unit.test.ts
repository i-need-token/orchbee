import { beforeAll, describe, expect, it } from "vitest";
import { encodePaste, inspectPaste } from "../paste-policy";
import type { WasmBridge } from "../../wasm-bridge";
import { initializeWasm } from "../../__tests__/test-helpers";

let bridge: WasmBridge;

beforeAll(async () => {
  bridge = await initializeWasm();
});

describe("paste policy", () => {
  it("replaces terminal control bytes", () => {
    const inspection = inspectPaste("hel\x1blo\0\x03world", { bracketed: true }, bridge);
    expect(inspection.sanitized).toBe("hel lo  world");
    expect(inspection.replacedControlBytes).toBe(3);
  });

  it("converts newlines only for non-bracketed paste", () => {
    const plain = inspectPaste(
      "first\nsecond",
      {
        bracketed: false,
        allowUnsafe: true,
      },
      bridge,
    );
    const bracketed = inspectPaste("first\nsecond", { bracketed: true }, bridge);
    expect(encodePaste(plain, bridge)).toBe("first\rsecond");
    expect(encodePaste(bracketed, bridge)).toBe("\x1b[200~first\nsecond\x1b[201~");
  });

  it("requires confirmation for multiline non-bracketed paste", () => {
    expect(inspectPaste("first\nsecond", { bracketed: false }, bridge).classification).toBe(
      "confirm",
    );
    expect(inspectPaste("first\nsecond", { bracketed: true }, bridge).classification).toBe("safe");
  });

  it("never trusts an embedded bracketed-paste terminator", () => {
    const inspection = inspectPaste(`safe\x1b[201~unsafe`, { bracketed: true }, bridge);
    expect(inspection.embeddedBracketTerminator).toBe(true);
    expect(inspection.classification).toBe("confirm");
    expect(inspection.sanitized).toBe("safe [201~unsafe");
  });

  it("supports disabling protection without disabling sanitization", () => {
    const inspection = inspectPaste(
      "first\n\x1bsecond",
      {
        bracketed: false,
        protection: false,
      },
      bridge,
    );
    expect(inspection.classification).toBe("safe");
    expect(inspection.sanitized).toBe("first\n second");
  });
});
