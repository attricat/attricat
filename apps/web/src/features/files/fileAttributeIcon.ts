import {
  FileIcon,
  FilesIcon,
  ImageIcon,
  ImagesIcon,
  type LucideIcon,
} from 'lucide-react';
import type { Attribute } from '../records/api';
import { attributeCardinalities } from '../records/constants';

/**
 * The icon that leads a file attribute's label, so a file field reads as one
 * at a glance: an image or a document, one or several.
 */
export const fileAttributeIcon = (
  attribute: Pick<Attribute, 'file_policy'>,
): LucideIcon => {
  const policy = attribute.file_policy;
  const many = policy?.cardinality === attributeCardinalities.many;
  if (policy?.image_only) return many ? ImagesIcon : ImageIcon;
  return many ? FilesIcon : FileIcon;
};
