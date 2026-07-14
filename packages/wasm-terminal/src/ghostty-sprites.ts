import type { DetailedFontMetrics } from "./font-metrics.js";

type LineStyle = "none" | "light" | "heavy" | "double";

interface BoxLines {
  readonly up?: LineStyle;
  readonly right?: LineStyle;
  readonly down?: LineStyle;
  readonly left?: LineStyle;
}

interface SpriteGeometry {
  readonly width: number;
  readonly height: number;
  readonly boxThickness: number;
}

const BOX_LINES = new Map<number, BoxLines>([
  [0x2500, { left: "light", right: "light" }],
  [0x2501, { left: "heavy", right: "heavy" }],
  [0x2502, { up: "light", down: "light" }],
  [0x250c, { right: "light", down: "light" }],
  [0x2510, { down: "light", left: "light" }],
  [0x2514, { up: "light", right: "light" }],
  [0x2518, { up: "light", left: "light" }],
  [0x252c, { right: "light", down: "light", left: "light" }],
  [0x2534, { up: "light", right: "light", left: "light" }],
  [0x253c, { up: "light", right: "light", down: "light", left: "light" }],
  [0x2551, { up: "double", down: "double" }],
]);

const POWERLINE_CODEPOINTS = new Set([0xe0b0, 0xe0b1, 0xe0b2, 0xe0b3]);
const CORNER_TRIANGLE_START = 0x25e2;
const CORNER_TRIANGLE_END = 0x25e5;
const BLOCK_ELEMENT_START = 0x2580;
const BLOCK_ELEMENT_END = 0x259f;

/** Return whether the built-in Ghostty sprite subset owns this codepoint. */
export function isGhosttySprite(codepoint: number): boolean {
  return (
    BOX_LINES.has(codepoint) ||
    POWERLINE_CODEPOINTS.has(codepoint) ||
    isBetween(codepoint, BLOCK_ELEMENT_START, BLOCK_ELEMENT_END) ||
    isBetween(codepoint, CORNER_TRIANGLE_START, CORNER_TRIANGLE_END)
  );
}

/** Draw a supported Ghostty sprite using the renderer's cell metrics. */
export function drawGhosttySprite(
  context: CanvasRenderingContext2D,
  codepoint: number,
  x: number,
  y: number,
  width: number,
  height: number,
  boxThickness: number,
): boolean {
  const geometry = {
    width: Math.max(1, Math.round(width)),
    height: Math.max(1, Math.round(height)),
    boxThickness: Math.max(1, Math.round(boxThickness)),
  } satisfies SpriteGeometry;
  const lines = BOX_LINES.get(codepoint);
  if (lines) {
    drawBoxLines(context, x, y, geometry, lines);
    return true;
  }
  if (POWERLINE_CODEPOINTS.has(codepoint)) {
    drawPowerline(context, codepoint, x, y, geometry);
    return true;
  }
  if (isBetween(codepoint, BLOCK_ELEMENT_START, BLOCK_ELEMENT_END)) {
    drawBlockElement(context, codepoint, x, y, geometry);
    return true;
  }
  if (isBetween(codepoint, CORNER_TRIANGLE_START, CORNER_TRIANGLE_END)) {
    drawCornerTriangle(context, codepoint, x, y, geometry);
    return true;
  }
  return false;
}

/** Convert the CSS-pixel terminal metrics to one physical sprite cell. */
export function physicalSpriteGeometry(
  metrics: DetailedFontMetrics,
  devicePixelRatio: number,
  cellWidth: number,
): SpriteGeometry {
  return {
    width: Math.max(1, Math.round(metrics.cellWidth * cellWidth * devicePixelRatio)),
    height: Math.max(1, Math.round(metrics.cellHeight * devicePixelRatio)),
    boxThickness: Math.max(1, Math.round(metrics.boxThickness * devicePixelRatio)),
  };
}

function drawBoxLines(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  geometry: SpriteGeometry,
  source: BoxLines,
): void {
  const lines = {
    up: source.up ?? "none",
    right: source.right ?? "none",
    down: source.down ?? "none",
    left: source.left ?? "none",
  } satisfies Required<BoxLines>;
  const { width, height, boxThickness: light } = geometry;
  const heavy = light * 2;

  const horizontalLightTop = saturatingHalf(height, light);
  const horizontalLightBottom = horizontalLightTop + light;
  const horizontalHeavyTop = saturatingHalf(height, heavy);
  const horizontalHeavyBottom = horizontalHeavyTop + heavy;
  const horizontalDoubleTop = Math.max(0, horizontalLightTop - light);
  const horizontalDoubleBottom = horizontalLightBottom + light;

  const verticalLightLeft = saturatingHalf(width, light);
  const verticalLightRight = verticalLightLeft + light;
  const verticalHeavyLeft = saturatingHalf(width, heavy);
  const verticalHeavyRight = verticalHeavyLeft + heavy;
  const verticalDoubleLeft = Math.max(0, verticalLightLeft - light);
  const verticalDoubleRight = verticalLightRight + light;

  let upBottom: number;
  if (lines.left === "heavy" || lines.right === "heavy") {
    upBottom = horizontalHeavyBottom;
  } else if (lines.left !== lines.right || lines.down === lines.up) {
    upBottom =
      lines.left === "double" || lines.right === "double"
        ? horizontalDoubleBottom
        : horizontalLightBottom;
  } else if (lines.left === "none" && lines.right === "none") {
    upBottom = horizontalLightBottom;
  } else {
    upBottom = horizontalLightTop;
  }

  let downTop: number;
  if (lines.left === "heavy" || lines.right === "heavy") {
    downTop = horizontalHeavyTop;
  } else if (lines.left !== lines.right || lines.up === lines.down) {
    downTop =
      lines.left === "double" || lines.right === "double"
        ? horizontalDoubleTop
        : horizontalLightTop;
  } else if (lines.left === "none" && lines.right === "none") {
    downTop = horizontalLightTop;
  } else {
    downTop = horizontalLightBottom;
  }

  let leftRight: number;
  if (lines.up === "heavy" || lines.down === "heavy") {
    leftRight = verticalHeavyRight;
  } else if (lines.up !== lines.down || lines.left === lines.right) {
    leftRight =
      lines.up === "double" || lines.down === "double" ? verticalDoubleRight : verticalLightRight;
  } else if (lines.up === "none" && lines.down === "none") {
    leftRight = verticalLightRight;
  } else {
    leftRight = verticalLightLeft;
  }

  let rightLeft: number;
  if (lines.up === "heavy" || lines.down === "heavy") {
    rightLeft = verticalHeavyLeft;
  } else if (lines.up !== lines.down || lines.right === lines.left) {
    rightLeft =
      lines.up === "double" || lines.down === "double" ? verticalDoubleLeft : verticalLightLeft;
  } else if (lines.up === "none" && lines.down === "none") {
    rightLeft = verticalLightLeft;
  } else {
    rightLeft = verticalLightRight;
  }

  if (lines.up === "light") {
    fillBox(context, x, y, verticalLightLeft, 0, verticalLightRight, upBottom);
  } else if (lines.up === "heavy") {
    fillBox(context, x, y, verticalHeavyLeft, 0, verticalHeavyRight, upBottom);
  } else if (lines.up === "double") {
    const leftBottom = lines.left === "double" ? horizontalLightTop : upBottom;
    const rightBottom = lines.right === "double" ? horizontalLightTop : upBottom;
    fillBox(context, x, y, verticalDoubleLeft, 0, verticalLightLeft, leftBottom);
    fillBox(context, x, y, verticalLightRight, 0, verticalDoubleRight, rightBottom);
  }

  if (lines.right === "light") {
    fillBox(context, x, y, rightLeft, horizontalLightTop, width, horizontalLightBottom);
  } else if (lines.right === "heavy") {
    fillBox(context, x, y, rightLeft, horizontalHeavyTop, width, horizontalHeavyBottom);
  } else if (lines.right === "double") {
    const topLeft = lines.up === "double" ? verticalLightRight : rightLeft;
    const bottomLeft = lines.down === "double" ? verticalLightRight : rightLeft;
    fillBox(context, x, y, topLeft, horizontalDoubleTop, width, horizontalLightTop);
    fillBox(context, x, y, bottomLeft, horizontalLightBottom, width, horizontalDoubleBottom);
  }

  if (lines.down === "light") {
    fillBox(context, x, y, verticalLightLeft, downTop, verticalLightRight, height);
  } else if (lines.down === "heavy") {
    fillBox(context, x, y, verticalHeavyLeft, downTop, verticalHeavyRight, height);
  } else if (lines.down === "double") {
    const leftTop = lines.left === "double" ? horizontalLightBottom : downTop;
    const rightTop = lines.right === "double" ? horizontalLightBottom : downTop;
    fillBox(context, x, y, verticalDoubleLeft, leftTop, verticalLightLeft, height);
    fillBox(context, x, y, verticalLightRight, rightTop, verticalDoubleRight, height);
  }

  if (lines.left === "light") {
    fillBox(context, x, y, 0, horizontalLightTop, leftRight, horizontalLightBottom);
  } else if (lines.left === "heavy") {
    fillBox(context, x, y, 0, horizontalHeavyTop, leftRight, horizontalHeavyBottom);
  } else if (lines.left === "double") {
    const topRight = lines.up === "double" ? verticalLightLeft : leftRight;
    const bottomRight = lines.down === "double" ? verticalLightLeft : leftRight;
    fillBox(context, x, y, 0, horizontalDoubleTop, topRight, horizontalLightTop);
    fillBox(context, x, y, 0, horizontalLightBottom, bottomRight, horizontalDoubleBottom);
  }
}

function drawPowerline(
  context: CanvasRenderingContext2D,
  codepoint: number,
  x: number,
  y: number,
  geometry: SpriteGeometry,
): void {
  const { width, height, boxThickness } = geometry;
  const points =
    codepoint === 0xe0b0 || codepoint === 0xe0b1
      ? ([
          [0, 0],
          [width, height / 2],
          [0, height],
        ] as const)
      : ([
          [width, 0],
          [0, height / 2],
          [width, height],
        ] as const);

  context.beginPath();
  context.moveTo(x + points[0][0], y + points[0][1]);
  context.lineTo(x + points[1][0], y + points[1][1]);
  context.lineTo(x + points[2][0], y + points[2][1]);
  if (codepoint === 0xe0b0 || codepoint === 0xe0b2) {
    context.closePath();
    context.fill();
    return;
  }
  context.save();
  context.lineCap = "butt";
  context.lineJoin = "miter";
  context.lineWidth = boxThickness;
  context.strokeStyle = context.fillStyle;
  context.stroke();
  context.restore();
}

function drawBlockElement(
  context: CanvasRenderingContext2D,
  codepoint: number,
  x: number,
  y: number,
  geometry: SpriteGeometry,
): void {
  const { width, height } = geometry;
  if (codepoint === 0x2580) {
    fillAlignedBlock(context, x, y, width, height, "upper", 1, 1 / 2);
  } else if (codepoint >= 0x2581 && codepoint <= 0x2587) {
    fillAlignedBlock(context, x, y, width, height, "lower", 1, (codepoint - 0x2580) / 8);
  } else if (codepoint === 0x2588) {
    fillBox(context, x, y, 0, 0, width, height);
  } else if (codepoint >= 0x2589 && codepoint <= 0x258f) {
    fillAlignedBlock(context, x, y, width, height, "left", (0x2590 - codepoint) / 8, 1);
  } else if (codepoint === 0x2590) {
    fillAlignedBlock(context, x, y, width, height, "right", 1 / 2, 1);
  } else if (codepoint >= 0x2591 && codepoint <= 0x2593) {
    fillShadedBlock(context, x, y, width, height, (codepoint - 0x2590) / 4);
  } else if (codepoint === 0x2594) {
    fillAlignedBlock(context, x, y, width, height, "upper", 1, 1 / 8);
  } else if (codepoint === 0x2595) {
    fillAlignedBlock(context, x, y, width, height, "right", 1 / 8, 1);
  } else {
    drawQuadrants(context, codepoint, x, y, width, height);
  }
}

function fillAlignedBlock(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  cellWidth: number,
  cellHeight: number,
  alignment: "upper" | "lower" | "left" | "right",
  widthFraction: number,
  heightFraction: number,
): void {
  const width = Math.round(cellWidth * widthFraction);
  const height = Math.round(cellHeight * heightFraction);
  const left = alignment === "right" ? cellWidth - width : 0;
  const top = alignment === "lower" ? cellHeight - height : 0;
  fillBox(context, x, y, left, top, left + width, top + height);
}

function fillShadedBlock(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  opacity: number,
): void {
  context.save();
  const sourceAlpha = Number.isFinite(context.globalAlpha) ? context.globalAlpha : 1;
  context.globalAlpha = sourceAlpha * opacity;
  fillBox(context, x, y, 0, 0, width, height);
  context.restore();
}

function drawQuadrants(
  context: CanvasRenderingContext2D,
  codepoint: number,
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  const masks = [0b0100, 0b1000, 0b0001, 0b1101, 0b1001, 0b0111, 0b1011, 0b0010, 0b0110, 0b1110];
  const mask = masks[codepoint - 0x2596] ?? 0;
  const leftHalfRight = Math.round(width / 2);
  const rightHalfLeft = width - Math.round(width / 2);
  const topHalfBottom = Math.round(height / 2);
  const bottomHalfTop = height - Math.round(height / 2);

  if ((mask & 0b0001) !== 0) {
    fillBox(context, x, y, 0, 0, leftHalfRight, topHalfBottom);
  }
  if ((mask & 0b0010) !== 0) {
    fillBox(context, x, y, rightHalfLeft, 0, width, topHalfBottom);
  }
  if ((mask & 0b0100) !== 0) {
    fillBox(context, x, y, 0, bottomHalfTop, leftHalfRight, height);
  }
  if ((mask & 0b1000) !== 0) {
    fillBox(context, x, y, rightHalfLeft, bottomHalfTop, width, height);
  }
}

function drawCornerTriangle(
  context: CanvasRenderingContext2D,
  codepoint: number,
  x: number,
  y: number,
  geometry: SpriteGeometry,
): void {
  const { width, height } = geometry;
  let points: readonly [
    readonly [number, number],
    readonly [number, number],
    readonly [number, number],
  ];
  if (codepoint === 0x25e2) {
    points = [
      [0, height],
      [width, height],
      [width, 0],
    ];
  } else if (codepoint === 0x25e3) {
    points = [
      [0, 0],
      [0, height],
      [width, height],
    ];
  } else if (codepoint === 0x25e4) {
    points = [
      [0, 0],
      [0, height],
      [width, 0],
    ];
  } else {
    points = [
      [0, 0],
      [width, height],
      [width, 0],
    ];
  }
  context.beginPath();
  context.moveTo(x + points[0][0], y + points[0][1]);
  context.lineTo(x + points[1][0], y + points[1][1]);
  context.lineTo(x + points[2][0], y + points[2][1]);
  context.closePath();
  context.fill();
}

function fillBox(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  left: number,
  top: number,
  right: number,
  bottom: number,
): void {
  if (right <= left || bottom <= top) return;
  context.fillRect(x + left, y + top, right - left, bottom - top);
}

function saturatingHalf(size: number, thickness: number): number {
  return Math.floor(Math.max(0, size - thickness) / 2);
}

function isBetween(value: number, start: number, end: number): boolean {
  return value >= start && value <= end;
}
