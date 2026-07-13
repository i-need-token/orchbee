import { afterEach, describe, expect, it, vi } from "vitest";
import { type RendererFrameTelemetry, WebGPURenderer } from "../renderer.js";
import { CellFlags, type GhosttyCell, type IRenderable } from "../types.js";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function createCanvas(
  getImageData?: CanvasRenderingContext2D["getImageData"],
  textMetrics: Partial<TextMetrics> = {},
  fillText: CanvasRenderingContext2D["fillText"] = () => {},
  fillRect: CanvasRenderingContext2D["fillRect"] = () => {},
): HTMLCanvasElement {
  const context = {
    clearRect: () => {},
    imageSmoothingEnabled: true,
    measureText: () =>
      ({
        width: 7.8,
        actualBoundingBoxAscent: 9.5,
        actualBoundingBoxDescent: 0,
        fontBoundingBoxAscent: 13,
        fontBoundingBoxDescent: 4,
        ...textMetrics,
      }) as TextMetrics,
    setTransform: () => {},
    fillRect,
    fillText,
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    stroke: () => {},
    setLineDash: () => {},
    drawImage: () => {},
    getImageData:
      getImageData ??
      ((_x: number, _y: number, width: number, height: number) =>
        ({ data: new Uint8ClampedArray(width * height * 4) }) as ImageData),
  };
  return {
    width: 0,
    height: 0,
    style: {},
    getContext: () => context,
  } as unknown as HTMLCanvasElement;
}

describe("WebGPURenderer font metrics", () => {
  it("uses full font bounds and preserves device pixel ratio", () => {
    vi.stubGlobal("window", { devicePixelRatio: 2 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas(), {
      fontFamily: "JetBrains Mono",
      fontSize: 13,
    });
    expect(renderer.getMetrics()).toEqual({ width: 8, height: 17, baseline: 13 });
    expect(renderer.getPalette()).toHaveLength(16);
    expect(renderer.charWidth).toBe(8);
    expect(renderer.charHeight).toBe(17);
    const requestRender = vi.fn<() => void>();
    renderer.setRenderRequestCallback(requestRender);
    renderer.setPreedit([{ text: "你", width: 2 }]);
    expect(requestRender).toHaveBeenCalledOnce();
    requestRender.mockClear();
    renderer.setSearchMatches([
      { text: "match", start: { column: 0, row: 0 }, end: { column: 4, row: 0 } },
    ]);
    expect(requestRender).toHaveBeenCalledOnce();
    renderer.dispose();
  });

  it("rounds fractional font metrics and centers the baseline like Ghostty", () => {
    const metrics = {
      width: 7.4,
      fontBoundingBoxAscent: 12.2,
      fontBoundingBoxDescent: 3.2,
    } satisfies Partial<TextMetrics>;
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas(undefined, metrics) });

    const renderer = new WebGPURenderer(createCanvas(undefined, metrics), { fontSize: 13 });

    expect(renderer.getMetrics()).toEqual({ width: 7, height: 15, baseline: 12 });
    renderer.dispose();
  });

  it("rebuilds physical-pixel resources when the device pixel ratio changes", async () => {
    vi.stubGlobal("window", { devicePixelRatio: 2 });
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const canvas = createCanvas();
    const renderer = new WebGPURenderer(canvas, { fontSize: 13, devicePixelRatio: 2 });
    await renderer.initGPU();
    const requestRender = vi.fn<() => void>();
    renderer.setRenderRequestCallback(requestRender);
    renderer.resize(10, 2);

    expect([canvas.width, canvas.height]).toEqual([160, 68]);
    expect(renderer.setDevicePixelRatio(2)).toBe(false);
    expect(renderer.setDevicePixelRatio(1.5)).toBe(true);
    expect(canvas.style.width).toBe("80px");
    expect(canvas.style.height).toBe("34px");
    expect([canvas.width, canvas.height]).toEqual([120, 51]);
    expect(requestRender).toHaveBeenCalledOnce();
    renderer.dispose();
  });

  it("stops cursor animation while suspended and resumes it once visible", () => {
    vi.useFakeTimers();
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas(), { cursorBlink: true });
    const requestRender = vi.fn<() => void>();
    renderer.setRenderRequestCallback(requestRender);

    vi.advanceTimersByTime(530);
    expect(requestRender).toHaveBeenCalledTimes(1);
    renderer.setSuspended(true);
    requestRender.mockClear();
    vi.advanceTimersByTime(1_060);
    expect(requestRender).not.toHaveBeenCalled();

    renderer.setSuspended(false);
    requestRender.mockClear();
    vi.advanceTimersByTime(530);
    expect(requestRender).toHaveBeenCalledTimes(1);
    renderer.dispose();
  });

  it("reports an observable Canvas 2D fallback when WebGPU is unavailable", async () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas());
    const health = vi.fn<(event: { state: string; backend: string; reason?: string }) => void>();
    renderer.setHealthChangeCallback(health);

    await expect(renderer.initGPU()).resolves.toBe(false);
    expect(health).toHaveBeenCalledWith({
      state: "fallback",
      backend: "canvas2d",
      reason: "webgpu-unavailable",
    });
    renderer.dispose();
  });

  it("animates blinking terminal text and suspends its timer while hidden", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas());
    await renderer.initGPU();
    const requestRender = vi.fn<() => void>();
    renderer.setRenderRequestCallback(requestRender);
    renderer.render(createBlinkingBuffer(), true);

    vi.advanceTimersByTime(530);
    expect(requestRender).toHaveBeenCalledOnce();
    renderer.setSuspended(true);
    requestRender.mockClear();
    vi.advanceTimersByTime(1_060);
    expect(requestRender).not.toHaveBeenCalled();
    renderer.dispose();
  });

  it("reports frame reasons, dirty work, uploads, and atlas pressure", async () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas());
    await renderer.initGPU();
    const telemetry = vi.fn<(frame: RendererFrameTelemetry) => void>();
    renderer.setFrameTelemetryCallback(telemetry);

    renderer.render(createBlinkingBuffer(), true);

    expect(telemetry).toHaveBeenCalledOnce();
    expect(telemetry.mock.calls[0]?.[0]).toMatchObject({
      frameId: 1,
      backend: "canvas2d",
      reasons: expect.arrayContaining(["full"]),
      forceAll: true,
      rowsRendered: 1,
      cellsVisited: 1,
      glyphInstances: 1,
      vertexUploadBytes: 0,
      atlasUploadBytes: 0,
      atlasGlyphCount: 0,
      atlasPressure: 0,
    });
    expect(renderer.getLastFrameTelemetry()?.durationMs).toBeGreaterThanOrEqual(0);
    renderer.dispose();
  });

  it("shapes bounded multi-cell runs and redraws individual cells when disabled", async () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("navigator", {});
    const fillText = vi.fn<CanvasRenderingContext2D["fillText"]>();
    vi.stubGlobal("document", {
      createElement: () => createCanvas(undefined, {}, fillText),
    });
    const renderer = new WebGPURenderer(createCanvas(undefined, {}, fillText));
    await renderer.initGPU();
    const buffer = createTextBuffer("=>");

    renderer.render(buffer, true);
    expect(fillText.mock.calls.map(([text]) => text)).toContain("=>");
    expect(renderer.getLastFrameTelemetry()).toMatchObject({
      shapingRuns: 1,
      glyphInstances: 1,
    });

    fillText.mockClear();
    renderer.setFontLigatures(false);
    renderer.render(buffer);
    expect(fillText.mock.calls.map(([text]) => text)).toEqual(expect.arrayContaining(["=", ">"]));
    expect(renderer.getLastFrameTelemetry()).toMatchObject({
      reasons: expect.arrayContaining(["font"]),
      shapingRuns: 0,
      glyphInstances: 2,
    });
    renderer.dispose();
  });

  it("renders Ghostty-style preedit text without marked-range inversion or a caret", () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    const fillText = vi.fn<CanvasRenderingContext2D["fillText"]>();
    const fillRect = vi.fn<CanvasRenderingContext2D["fillRect"]>();
    vi.stubGlobal("document", {
      createElement: () => createCanvas(undefined, {}, fillText, fillRect),
    });
    const canvas = createCanvas(undefined, {}, fillText, fillRect);
    const renderer = new WebGPURenderer(canvas);
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Missing Canvas 2D context");
    const renderPreedit = Reflect.get(renderer, "renderPreedit2D") as (
      context: CanvasRenderingContext2D,
      layout: {
        readonly cells: readonly { readonly text: string; readonly width: number }[];
        readonly start: number;
        readonly end: number;
        readonly row: number;
        readonly width: number;
        readonly selectionStart: number;
        readonly selectionEnd: number;
      },
    ) => void;

    renderPreedit.call(renderer, context, {
      cells: [
        { text: "c", width: 1 },
        { text: "h", width: 1 },
        { text: "e", width: 1 },
      ],
      start: 0,
      end: 3,
      row: 0,
      width: 3,
      selectionStart: 0,
      selectionEnd: 2,
    });

    expect(fillText.mock.calls.map(([text]) => text)).toEqual(["c", "h", "e"]);
    expect(fillRect.mock.calls).toEqual([
      [0, 0, 24, 17],
      [0, 14, 24, 1],
    ]);
    renderer.dispose();
  });

  it("renders bar and underline cursors at one physical pixel", () => {
    vi.stubGlobal("window", { devicePixelRatio: 2 });
    const fillRect = vi.fn<CanvasRenderingContext2D["fillRect"]>();
    vi.stubGlobal("document", {
      createElement: () => createCanvas(undefined, {}, undefined, fillRect),
    });
    const canvas = createCanvas(undefined, {}, undefined, fillRect);
    const renderer = new WebGPURenderer(canvas, { devicePixelRatio: 2 });
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Missing Canvas 2D context");
    const renderCursor = Reflect.get(renderer, "renderCursor2D") as (
      context: CanvasRenderingContext2D,
      x: number,
      y: number,
      style: "block" | "block_hollow" | "underline" | "bar",
    ) => void;

    renderCursor.call(renderer, context, 2, 1, "bar");
    renderCursor.call(renderer, context, 2, 1, "underline");

    expect(fillRect.mock.calls).toEqual([
      [15.5, 17, 0.5, 17],
      [16, 31, 8, 0.5],
    ]);
    renderer.dispose();
  });

  it("renders the unfocused hollow cursor border at one physical pixel", () => {
    vi.stubGlobal("window", { devicePixelRatio: 2 });
    const fillRect = vi.fn<CanvasRenderingContext2D["fillRect"]>();
    vi.stubGlobal("document", {
      createElement: () => createCanvas(undefined, {}, undefined, fillRect),
    });
    const canvas = createCanvas(undefined, {}, undefined, fillRect);
    const renderer = new WebGPURenderer(canvas, { devicePixelRatio: 2 });
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Missing Canvas 2D context");
    const renderCursor = Reflect.get(renderer, "renderCursor2D") as (
      context: CanvasRenderingContext2D,
      x: number,
      y: number,
      style: "block" | "block_hollow" | "underline" | "bar",
    ) => void;

    renderCursor.call(renderer, context, 2, 1, "block_hollow");

    expect(fillRect.mock.calls).toEqual([
      [16, 17, 8, 0.5],
      [16, 33.5, 8, 0.5],
      [16, 17, 0.5, 17],
      [23.5, 17, 0.5, 17],
    ]);
    renderer.dispose();
  });

  it("requests a redraw only when terminal focus changes", () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas());
    const requestRender = vi.fn<() => void>();
    renderer.setRenderRequestCallback(requestRender);

    renderer.setFocused(false);
    renderer.setFocused(true);
    renderer.setFocused(true);
    renderer.setFocused(false);

    expect(requestRender).toHaveBeenCalledTimes(2);
    renderer.dispose();
  });

  it("uploads only the merged glyph-atlas dirty rectangle", () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas());
    const writeTexture = vi.fn<GPUQueue["writeTexture"]>();
    const device = {
      queue: { writeTexture },
      destroy: () => {},
    } as unknown as GPUDevice;
    const texture = { destroy: () => {} } as unknown as GPUTexture;
    Reflect.set(renderer, "device", device);
    Reflect.set(renderer, "atlasTexture", texture);

    const markAtlasDirty = Reflect.get(renderer, "markAtlasDirty") as (
      x: number,
      y: number,
      width: number,
      height: number,
    ) => void;
    const uploadAtlas = Reflect.get(renderer, "uploadAtlas") as () => void;
    markAtlasDirty.call(renderer, 10, 12, 8, 17);
    markAtlasDirty.call(renderer, 30, 8, 4, 3);
    uploadAtlas.call(renderer);

    expect(writeTexture).toHaveBeenCalledWith(
      { texture, origin: { x: 10, y: 8 } },
      expect.any(Uint8ClampedArray),
      { bytesPerRow: 96, rowsPerImage: 21 },
      { width: 24, height: 21 },
    );
    expect(Reflect.get(renderer, "frameAtlasUploadBytes")).toBe(2_016);
    expect(Reflect.get(renderer, "atlasDirtyBounds")).toBeNull();
    renderer.dispose();
  });

  it("preserves colored emoji while leaving ordinary glyphs tintable", () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    const getImageData = vi.fn<CanvasRenderingContext2D["getImageData"]>(() => {
      return { data: new Uint8ClampedArray([255, 90, 20, 255]) } as ImageData;
    });
    vi.stubGlobal("document", { createElement: () => createCanvas(getImageData) });
    const renderer = new WebGPURenderer(createCanvas(getImageData));
    const ensureGlyph = Reflect.get(renderer, "ensureGlyph") as (
      text: string,
      flags: number,
      width: number,
    ) => { readonly colored: boolean } | null;

    expect(ensureGlyph.call(renderer, "A", 0, 1)?.colored).toBe(false);
    expect(ensureGlyph.call(renderer, "🚀", 0, 2)?.colored).toBe(true);
    expect(getImageData).toHaveBeenCalledOnce();
    renderer.dispose();
  });

  it("preserves glyph ink bounds and bearings in atlas geometry", () => {
    const metrics = {
      actualBoundingBoxLeft: 2,
      actualBoundingBoxRight: 9,
      actualBoundingBoxAscent: 11,
      actualBoundingBoxDescent: 3,
    } satisfies Partial<TextMetrics>;
    vi.stubGlobal("window", { devicePixelRatio: 2 });
    vi.stubGlobal("document", { createElement: () => createCanvas(undefined, metrics) });
    const renderer = new WebGPURenderer(createCanvas(undefined, metrics), {
      fontFamily: "Menlo",
      fontSize: 13,
      devicePixelRatio: 2,
    });
    const ensureGlyph = Reflect.get(renderer, "ensureGlyph") as (
      text: string,
      flags: number,
      width: number,
    ) => {
      readonly w: number;
      readonly h: number;
      readonly offsetX: number;
      readonly offsetY: number;
      readonly renderWidth: number;
      readonly renderHeight: number;
    } | null;

    expect(ensureGlyph.call(renderer, "j", CellFlags.ITALIC, 1)).toMatchObject({
      w: 15,
      h: 18,
      offsetX: -2,
      offsetY: 6.5,
      renderWidth: 7.5,
      renderHeight: 9,
    });
    renderer.dispose();
  });

  it("preserves rasterization state when the glyph atlas grows", () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    const renderer = new WebGPURenderer(createCanvas(), {
      fontFamily: "Menlo",
      fontSize: 13,
    });
    const atlasCanvas = Reflect.get(renderer, "atlasCanvas") as HTMLCanvasElement;
    const previousContext = atlasCanvas.getContext("2d");
    if (!previousContext) throw new Error("Missing initial atlas context");
    previousContext.font = "italic bold 13px Menlo";
    previousContext.textAlign = "left";
    previousContext.textBaseline = "alphabetic";
    previousContext.direction = "rtl";
    const growAtlas = Reflect.get(renderer, "growAtlas") as () => boolean;

    expect(growAtlas.call(renderer)).toBe(true);
    const grownCanvas = Reflect.get(renderer, "atlasCanvas") as HTMLCanvasElement;
    const grownContext = grownCanvas.getContext("2d");
    expect(grownContext).toMatchObject({
      font: "italic bold 13px Menlo",
      textAlign: "left",
      textBaseline: "alphabetic",
      direction: "rtl",
    });
    renderer.dispose();
  });

  it("rebuilds WebGPU resources after a device-loss event", async () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    vi.stubGlobal("GPUTextureUsage", { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
    vi.stubGlobal("GPUBufferUsage", { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
    const first = createControllableDevice();
    const second = createControllableDevice();
    const requestDevice = vi
      .fn<() => Promise<GPUDevice>>()
      .mockResolvedValueOnce(first.device)
      .mockResolvedValueOnce(second.device);
    const requestAdapter = vi.fn<() => Promise<GPUAdapter>>(async () =>
      Promise.resolve({ requestDevice } as unknown as GPUAdapter),
    );
    vi.stubGlobal("navigator", {
      gpu: { requestAdapter, getPreferredCanvasFormat: () => "bgra8unorm" },
    });
    const context = {
      configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
    } as unknown as GPUCanvasContext;
    const renderer = new WebGPURenderer(createWebGPUCanvas(context));
    const health = vi.fn<(event: { state: string }) => void>();
    renderer.setHealthChangeCallback(health);

    await expect(renderer.initGPU()).resolves.toBe(true);
    first.lose({ reason: "unknown", message: "simulated device loss" });
    await vi.waitFor(() => expect(requestDevice).toHaveBeenCalledTimes(2));
    expect(renderer.getBackend()).toBe("webgpu");

    expect(health.mock.calls.map(([event]) => event.state)).toEqual([
      "ready",
      "lost",
      "recovering",
      "ready",
    ]);
    expect(context.configure).toHaveBeenCalledTimes(2);
    renderer.dispose();
  });

  it("reports a failed WebGPU recovery without attempting an invalid Canvas fallback", async () => {
    vi.stubGlobal("window", { devicePixelRatio: 1 });
    vi.stubGlobal("document", { createElement: () => createCanvas() });
    vi.stubGlobal("GPUTextureUsage", { TEXTURE_BINDING: 1, COPY_DST: 2, RENDER_ATTACHMENT: 4 });
    vi.stubGlobal("GPUBufferUsage", { UNIFORM: 1, COPY_DST: 2, VERTEX: 4 });
    const first = createControllableDevice();
    const adapter = { requestDevice: async () => first.device } as unknown as GPUAdapter;
    const requestAdapter = vi
      .fn<() => Promise<GPUAdapter | null>>()
      .mockResolvedValueOnce(adapter)
      .mockResolvedValueOnce(null);
    vi.stubGlobal("navigator", {
      gpu: { requestAdapter, getPreferredCanvasFormat: () => "bgra8unorm" },
    });
    const context = {
      configure: vi.fn<(configuration: GPUCanvasConfiguration) => void>(),
    } as unknown as GPUCanvasContext;
    const renderer = new WebGPURenderer(createWebGPUCanvas(context));
    const health = vi.fn<(event: { state: string; reason?: string }) => void>();
    renderer.setHealthChangeCallback(health);

    await renderer.initGPU();
    first.lose({ reason: "unknown", message: "simulated device loss" });
    await vi.waitFor(() =>
      expect(health.mock.calls.at(-1)?.[0]).toMatchObject({
        state: "recovery-failed",
        reason: "No WebGPU adapter is available",
      }),
    );

    expect(renderer.getBackend()).toBe("uninitialized");
    expect(context.configure).toHaveBeenCalledOnce();
    renderer.dispose();
  });
});

function createWebGPUCanvas(context: GPUCanvasContext): HTMLCanvasElement {
  const canvas = createCanvas();
  const getContext = canvas.getContext.bind(canvas);
  return {
    ...canvas,
    getContext: ((kind: string, options?: unknown) =>
      kind === "webgpu" ? context : getContext(kind, options)) as HTMLCanvasElement["getContext"],
  } as HTMLCanvasElement;
}

function createControllableDevice(): {
  readonly device: GPUDevice;
  lose(info: { readonly reason: GPUDeviceLostReason; readonly message: string }): void;
} {
  let resolveLost: ((info: GPUDeviceLostInfo) => void) | undefined;
  const lost = new Promise<GPUDeviceLostInfo>((resolve) => {
    resolveLost = resolve;
  });
  const destroy = (): void =>
    resolveLost?.({ reason: "destroyed", message: "destroyed" } as GPUDeviceLostInfo);
  const device = {
    lost,
    destroy,
    pushErrorScope: () => {},
    popErrorScope: async () => null,
    createShaderModule: () => ({}),
    createRenderPipeline: () => ({ getBindGroupLayout: () => ({}) }),
    createTexture: () => ({ destroy: () => {}, createView: () => ({}) }),
    createSampler: () => ({}),
    createBuffer: () => ({ destroy: () => {} }),
  } as unknown as GPUDevice;
  return {
    device,
    lose: (info) => resolveLost?.(info as GPUDeviceLostInfo),
  };
}

function createBlinkingBuffer(): IRenderable {
  const cell: GhosttyCell = {
    codepoint: 88,
    fg_r: 255,
    fg_g: 255,
    fg_b: 255,
    bg_r: 0,
    bg_g: 0,
    bg_b: 0,
    flags: CellFlags.BLINK,
    width: 1,
    hyperlink_id: 0,
    grapheme_len: 0,
    selected: false,
    underline_style: 0,
    underline_r: -1,
    underline_g: -1,
    underline_b: -1,
    overline: false,
  };
  return {
    getLine: () => [cell],
    getCursor: () => ({
      x: 0,
      y: 0,
      viewportX: 0,
      viewportY: 0,
      visible: false,
      blinking: false,
      style: "block",
    }),
    getDimensions: () => ({ cols: 1, rows: 1 }),
    isRowDirty: () => true,
    clearDirty: () => {},
  };
}

function createTextBuffer(text: string): IRenderable {
  const cells = [...text].map(
    (character): GhosttyCell => ({
      codepoint: character.codePointAt(0) ?? 0,
      fg_r: 255,
      fg_g: 255,
      fg_b: 255,
      bg_r: 0,
      bg_g: 0,
      bg_b: 0,
      flags: 0,
      width: 1,
      hyperlink_id: 0,
      grapheme_len: 0,
      selected: false,
      underline_style: 0,
      underline_r: -1,
      underline_g: -1,
      underline_b: -1,
      overline: false,
    }),
  );
  return {
    getLine: (row) => (row === 0 ? cells : null),
    getCursor: () => ({
      x: 0,
      y: 0,
      viewportX: 0,
      viewportY: 0,
      visible: false,
      blinking: false,
      style: "block",
    }),
    getDimensions: () => ({ cols: cells.length, rows: 1 }),
    isRowDirty: () => true,
    clearDirty: () => {},
  };
}
