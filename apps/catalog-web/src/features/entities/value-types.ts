export const attributeValueTypes = {
  string: 'string',
  number: 'number',
  integer: 'integer',
  boolean: 'boolean',
  date: 'date',
  datetime: 'datetime',
  time: 'time',
  file: 'file',
  relationship: 'relationship',
} as const;

export const attributeValueKinds = {
  scalar: 'scalar',
  relationship: 'relationship',
} as const;
