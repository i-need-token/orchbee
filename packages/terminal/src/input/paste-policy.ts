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

export interface PasteCodec {
  isPasteSafe(text: string): boolean;
  encodePaste(text: string, bracketed: boolean): Uint8Array;
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

function inspectSanitization(
  text: string,
  codec: PasteCodec,
): {
  readonly sanitized: string;
  readonly replacedControlBytes: number;
} {
  const bracketed = new TextDecoder().decode(codec.encodePaste(text, true));
  if (!bracketed.startsWith("\x1b[200~") || !bracketed.endsWith(BRACKETED_PASTE_END)) {
    throw new Error("Ghostty returned an invalid bracketed-paste encoding");
  }
  const sanitized = bracketed.slice(6, -BRACKETED_PASTE_END.length);
  const originalCharacters = [...text];
  const sanitizedCharacters = [...sanitized];
  let replacedControlBytes = 0;
  const count = Math.max(originalCharacters.length, sanitizedCharacters.length);
  for (let index = 0; index < count; index++) {
    if (originalCharacters[index] !== sanitizedCharacters[index]) replacedControlBytes++;
  }
  return { sanitized, replacedControlBytes };
}

export function inspectPaste(
  text: string,
  options: PastePolicyOptions,
  codec: PasteCodec,
): PasteInspection {
  const multiline = text.includes("\n");
  const embeddedBracketTerminator = text.includes(BRACKETED_PASTE_END);
  const { sanitized, replacedControlBytes } = inspectSanitization(text, codec);
  const protection = options.protection ?? true;
  const bracketedSafe = options.bracketedSafe ?? true;

  let classification: PasteInspection["classification"] = "safe";
  if (protection && !options.allowUnsafe && !codec.isPasteSafe(text)) {
    const protectedByBracketedPaste =
      options.bracketed && bracketedSafe && multiline && !embeddedBracketTerminator;
    if (!protectedByBracketedPaste) classification = "confirm";
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

export function encodePaste(inspection: PasteInspection, codec: PasteCodec): string {
  return new TextDecoder().decode(codec.encodePaste(inspection.original, inspection.bracketed));
}
