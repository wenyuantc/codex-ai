import { describe, expect, it } from "vitest";

import { fitContextMenuToViewport } from "@/lib/contextMenuPosition";

const viewport = { viewportWidth: 1280, viewportHeight: 720 };

describe("fitContextMenuToViewport", () => {
  it("opens below the click when there is enough space", () => {
    const result = fitContextMenuToViewport({
      originX: 200,
      originY: 80,
      width: 192,
      height: 260,
      ...viewport,
    });

    expect(result).toMatchObject({ x: 200, y: 80, placedAbove: false, maxHeight: 704 });
  });

  it("flips above the click when the bottom would clip", () => {
    const result = fitContextMenuToViewport({
      originX: 240,
      originY: 560,
      width: 192,
      height: 380,
      ...viewport,
    });

    expect(result.placedAbove).toBe(true);
    expect(result.y).toBe(180);
    expect(result.y + 380).toBe(560);
  });

  it("keeps the menu inside the bottom padding when clicked near the edge", () => {
    const height = 300;
    const result = fitContextMenuToViewport({
      originX: 100,
      originY: 700,
      width: 192,
      height,
      ...viewport,
    });

    expect(result.placedAbove).toBe(true);
    expect(result.y).toBe(400);
    expect(result.y + height).toBeLessThanOrEqual(712);
  });

  it("stays below a top-edge click instead of flipping off-screen", () => {
    const result = fitContextMenuToViewport({
      originX: 80,
      originY: 12,
      width: 192,
      height: 280,
      ...viewport,
    });

    expect(result.placedAbove).toBe(false);
    expect(result.y).toBe(12);
  });

  it("shifts left when the menu would overflow the right edge", () => {
    const result = fitContextMenuToViewport({
      originX: 1200,
      originY: 80,
      width: 192,
      height: 120,
      ...viewport,
    });

    expect(result.x).toBe(1080);
    expect(result.x).toBeGreaterThanOrEqual(8);
    expect(result.x + 192).toBeLessThanOrEqual(1272);
  });

  it("clamps a viewport-taller menu to padding with a scrollable maxHeight", () => {
    const result = fitContextMenuToViewport({
      originX: 40,
      originY: 400,
      width: 192,
      height: 900,
      ...viewport,
    });

    expect(result.maxHeight).toBe(704);
    expect(result.y).toBe(8);
    expect(result.y + result.maxHeight).toBe(712);
  });
});
