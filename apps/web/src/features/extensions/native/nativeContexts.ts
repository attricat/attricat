import { createContext } from 'react';

/** Which way the nearest UI kit stack lays its children out. */
export const nativeDirections = {
  vertical: 'vertical',
  horizontal: 'horizontal',
} as const;
export type NativeDirection =
  (typeof nativeDirections)[keyof typeof nativeDirections];

/** Inputs fill a column but keep their own width in a row. */
export const NativeDirectionContext = createContext<NativeDirection>(
  nativeDirections.vertical,
);

/**
 * The label of the host field an attribute editor fills. The first input
 * that hides its own label draws this one on its outline instead, as every
 * other field does; the host then keeps its label for assistive technology
 * only.
 */
export type NativeFieldLabel = {
  text: string;
  required: boolean;
  error: boolean;
  /** Grants the label to the input `id` unless another input holds it. */
  claim: (id: string, grant: (owns: boolean) => void) => void;
  release: (id: string) => void;
};

export const NativeFieldLabelContext = createContext<
  NativeFieldLabel | undefined
>(undefined);
