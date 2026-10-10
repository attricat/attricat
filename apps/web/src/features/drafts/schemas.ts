import { z } from 'zod';

/** A TOML definition kept by the source editors. */
export const definitionDraftSchema = z.string();

/** Scalar and relationship field values keyed by attribute code. */
export const formFieldsDraftSchema = z.record(z.string(), z.string());
