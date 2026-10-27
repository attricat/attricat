/** The Attricat mark alone, or with the wordmark beside it. */
export type BrandVariant = 'mark' | 'wordmark';

export const brandIconHeight = 32;
export const brandIconWidths = {
  mark: 32,
  wordmark: 161,
} as const satisfies Record<BrandVariant, number>;
