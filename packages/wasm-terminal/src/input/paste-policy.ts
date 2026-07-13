export type PasteSource = "clipboard" | "beforeinput" | "drop" | "selection" | "api";

export interface PastePolicyOptions {
  readonly bracketed: boolean;
  readonly protection?: boolean;
  readonly bracketedSafe?: boolean;
  readonly allowUnsafe?: boolean;
}

export interface PasteInspection {
  readonly original: string;
  readonly sanitized: string;
  readonly bracketed: boolean;
  readonly multiline: boolean;
  readonly embeddedBracketTerminator: boolean;
  readonly replacedControlBytes: number;
  readonly classification: "safe" | "confirm";
}

export type PasteResult =
  | {
      readonly kind: "emitted";
      readonly bytes: number;
      readonly source: PasteSource;
      readonly inspection: PasteInspection;
    }
  | { readonly kind: "empty"; readonly source: PasteSource; readonly inspection: PasteInspection }
  | {
      readonly kind: "cancelled";
      readonly source: PasteSource;
      readonly inspection: PasteInspection;
    }
  | {
      readonly kind: "blocked";
      readonly reason: string;
      readonly source: PasteSource;
      readonly inspection: PasteInspection;
    };

const BRACKETED_PASTE_END = "\x1b[201~";

const REPLACED_CONTROL_BYTES = new Set([
  0x00, 0x03, 0x04, 0x05, 0x08, 0x0f, 0x11, 0x12, 0x13, 0x15, 0x16, 0x17, 0x1a, 0x1b, 0x1c, 0x7f,
]);

function sanitizePasteText(text: string): {
  readonly sanitized: string;
  readonly replacedControlBytes: number;
} {
  let sanitized = "";
  let replacedControlBytes = 0;
  for (const character of text) {
    const codepoint = character.codePointAt(0);
    if (codepoint !== undefined && REPLACED_CONTROL_BYTES.has(codepoint)) {
      sanitized += " ";
      replacedControlBytes++;
    } else {
      sanitized += character;
    }
  }
  return { sanitized, replacedControlBytes };
}

export function inspectPaste(text: string, options: PastePolicyOptions): PasteInspection {
  const multiline = text.includes("\n");
  const embeddedBracketTerminator = text.includes(BRACKETED_PASTE_END);
  const { sanitized, replacedControlBytes } = sanitizePasteText(text);
  const protection = options.protection ?? true;
  const bracketedSafe = options.bracketedSafe ?? true;

  let classification: PasteInspection["classification"] = "safe";
  if (protection && !options.allowUnsafe) {
    if (options.bracketed && embeddedBracketTerminator) classification = "confirm";
    else if (!options.bracketed || !bracketedSafe) {
      if (multiline || embeddedBracketTerminator) classification = "confirm";
    }
  }

  return {
    original: text,
    sanitized,
    bracketed: options.bracketed,
    multiline,
    embeddedBracketTerminator,
    replacedControlBytes,
    classification,
  };
}

export function encodePaste(inspection: PasteInspection): string {
  if (inspection.bracketed) {
    return `\x1b[200~${inspection.sanitized}\x1b[201~`;
  }
  return inspection.sanitized.replaceAll("\n", "\r");
}
