import type { Attribute } from './api';

/** Every blueprint a relationship may target; empty allows any blueprint. */
export const allowedTargetBlueprints = (
  attribute: Pick<
    Attribute,
    'target_blueprint_code' | 'target_blueprint_codes'
  >,
): string[] =>
  attribute.target_blueprint_codes?.length
    ? attribute.target_blueprint_codes
    : attribute.target_blueprint_code
      ? [attribute.target_blueprint_code]
      : [];
