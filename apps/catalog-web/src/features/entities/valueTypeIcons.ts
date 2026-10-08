import {
  BracesIcon,
  CalendarClockIcon,
  CalendarIcon,
  ClockIcon,
  FileIcon,
  HashIcon,
  ToggleLeftIcon,
  TypeIcon,
  type LucideIcon,
} from 'lucide-react';
import { RelationshipIcon } from '../../components/systemIcons';
import { attributeValueTypes } from './valueTypes';

export type AttributeValueType =
  (typeof attributeValueTypes)[keyof typeof attributeValueTypes];

export const valueTypeIcons = {
  [attributeValueTypes.string]: TypeIcon,
  [attributeValueTypes.number]: HashIcon,
  [attributeValueTypes.integer]: HashIcon,
  [attributeValueTypes.boolean]: ToggleLeftIcon,
  [attributeValueTypes.date]: CalendarIcon,
  [attributeValueTypes.datetime]: CalendarClockIcon,
  [attributeValueTypes.time]: ClockIcon,
  [attributeValueTypes.json]: BracesIcon,
  [attributeValueTypes.file]: FileIcon,
  [attributeValueTypes.relationship]: RelationshipIcon,
} as const satisfies Record<AttributeValueType, LucideIcon>;

export const valueTypeLabelKey = (valueType: AttributeValueType) =>
  `entities.valueTypes.${valueType}` as const;
