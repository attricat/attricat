import type { AttributeContext } from './api';
import { defaultContextCode } from './constants';

/** A context's code, or `defaultLabel` for the default context. */
export const contextLabel = (
  context: AttributeContext,
  defaultLabel: string,
) => (context.code === defaultContextCode ? defaultLabel : context.code);
