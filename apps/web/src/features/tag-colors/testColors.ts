/** `#RRGGBB` as the `rgb()` jsdom reports for computed colors. */
export const hexToRgb = (hex: string) => {
  const [r, g, b] = [1, 3, 5].map((start) =>
    Number.parseInt(hex.slice(start, start + 2), 16),
  );
  return `rgb(${r}, ${g}, ${b})`;
};
