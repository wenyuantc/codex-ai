export const CONTEXT_MENU_VIEWPORT_PADDING = 8;

export interface FitContextMenuToViewportInput {
  originX: number;
  originY: number;
  width: number;
  height: number;
  viewportWidth: number;
  viewportHeight: number;
  padding?: number;
}

export interface FitContextMenuToViewportResult {
  x: number;
  y: number;
  maxHeight: number;
  placedAbove: boolean;
}

export function fitContextMenuToViewport(
  input: FitContextMenuToViewportInput,
): FitContextMenuToViewportResult {
  const padding = input.padding ?? CONTEXT_MENU_VIEWPORT_PADDING;
  const { originX, originY, width, viewportWidth, viewportHeight } = input;
  const maxHeight = Math.max(0, viewportHeight - padding * 2);
  const height = Math.min(Math.max(0, input.height), maxHeight);

  let x = originX;
  if (x + width > viewportWidth - padding) {
    x = viewportWidth - padding - width;
  }
  x = Math.max(padding, x);

  const spaceBelow = viewportHeight - padding - originY;
  const spaceAbove = originY - padding;
  let y: number;
  let placedAbove = false;

  if (height <= spaceBelow) {
    y = originY;
  } else if (height <= spaceAbove) {
    y = originY - height;
    placedAbove = true;
  } else if (spaceAbove > spaceBelow) {
    y = padding;
    placedAbove = true;
  } else {
    y = Math.max(padding, viewportHeight - padding - height);
  }

  return { x, y, maxHeight, placedAbove };
}
