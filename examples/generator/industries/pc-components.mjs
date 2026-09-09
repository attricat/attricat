export const id = "pc-components";
export const schemaVersion = 1;

export const profiles = {
  micro: { families: 199, skus: 796, categories: 4, manufacturers: 1 },
  small: { families: 1990, skus: 7960, categories: 40, manufacturers: 10 },
  medium: { families: 19900, skus: 79600, categories: 400, manufacturers: 100 },
  large: {
    families: 199000,
    skus: 796000,
    categories: 4000,
    manufacturers: 1000,
  },
};

const blueprint = (
  code,
  name,
  attributes,
  tableColumns = '[{ field = "name", label = "Name" }]',
) => `format_version = 1
code = "${code}"
name = "${name}"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["name"]

[views.table]
type = "table"
columns = ${tableColumns}

${attributes}`;

export const blueprints = (prefix) => ({
  manufacturer: blueprint(
    `${prefix}_manufacturer`,
    "PC manufacturer",
    `[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "vendor_code"
value_type = "string"`,
  ),
  category: blueprint(
    `${prefix}_category`,
    "PC category",
    `[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "slug"
value_type = "string"

[[attributes]]
code = "parent_category"
value_type = "relationship"
target_blueprint = "${prefix}_category"
context_fallback = "none"`,
  ),
  family: blueprint(
    `${prefix}_family`,
    "PC product family",
    `[[attributes]]
code = "name"
value_type = "string"
tags = ["display", "searchable"]

[[attributes]]
code = "family_code"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"

[[attributes]]
code = "product_type"
value_type = "string"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "${prefix}_category"

[[attributes]]
code = "manufacturer"
value_type = "relationship"
target_blueprint = "${prefix}_manufacturer"`,
  ),
  sku: blueprint(
    `${prefix}_sku`,
    "PC sellable SKU",
    `[[attributes]]
code = "name"
value_type = "string"
tags = ["display", "searchable"]

[[attributes]]
code = "sku"
value_type = "string"

[[attributes]]
code = "description"
value_type = "string"

[[attributes]]
code = "product_type"
value_type = "string"

[[attributes]]
code = "price"
value_type = "number"

[[attributes]]
code = "stock_on_hand"
value_type = "number"

[[attributes]]
code = "availability"
value_type = "string"

[[attributes]]
code = "interface"
value_type = "string"

[[attributes]]
code = "capacity"
value_type = "string"

[[attributes]]
code = "form_factor"
value_type = "string"

[[attributes]]
code = "wattage"
value_type = "number"

[[attributes]]
code = "family"
value_type = "relationship"
target_blueprint = "${prefix}_family"

[[attributes]]
code = "category"
value_type = "relationship"
target_blueprint = "${prefix}_category"

[[attributes]]
code = "manufacturer"
value_type = "relationship"
target_blueprint = "${prefix}_manufacturer"

[[attributes]]
code = "compatible_skus"
value_type = "relationship"
target_blueprint = "${prefix}_sku"
context_fallback = "none"

[[attributes]]
code = "product_files"
value_type = "file"
cardinality = "many"
allowed_mime_groups = ["image", "text", "application"]
allowed_extensions = ["png", "pdf", "txt"]
max_bytes = 1048576`,
    `[
  { field = "name", label = "Product" },
  { field = "sku", label = "SKU" },
  { field = "product_type", label = "Type" },
  { field = "price", label = "Price" },
  { field = "stock_on_hand", label = "Stock" },
  { field = "availability", label = "Availability" },
  { field = "interface", label = "Interface" },
  { field = "capacity", label = "Capacity" },
  { field = "form_factor", label = "Form factor" },
  { field = "wattage", label = "Wattage" },
]`,
  ),
});

const vendors = [
  "Kestrel Microdevices",
  "Meridian Boardworks",
  "Solvane Storage",
  "Asterion Memory",
  "Voltarra Power",
  "Ridgepoint Thermal",
  "Noric Display",
  "Helix Networking",
  "Cobalt Peripherals",
  "Ternion Systems",
];
const types = [
  ["Graphics Card", "PCIe 4.0", "ATX", 285],
  ["Motherboard", "PCIe 5.0", "ATX", 65],
  ["Solid State Drive", "NVMe", "M.2 2280", 8],
  ["Memory Kit", "DDR5", "DIMM", 12],
  ["Power Supply", "ATX 3.1", "ATX", 850],
  ["CPU Cooler", "PWM", "Tower", 6],
  ["Network Adapter", "PCIe", "Low profile", 9],
  ["Computer Case", "USB-C", "Mid tower", 0],
];
const capacity = ["8 GB", "16 GB", "32 GB", "64 GB", "1 TB", "2 TB", "4 TB"];

export const categoryFor = (index) => ({
  name: `${["Components", "Storage", "Memory", "Networking", "Cooling", "Cases", "Power", "Accessories"][index % 8]} ${String(index + 1).padStart(4, "0")}`,
  slug: `pc-components-${String(index + 1).padStart(5, "0")}`,
  parentIndex: index === 0 ? null : index % Math.min(8, index),
});

export const manufacturerFor = (index) => ({
  name:
    index < vendors.length
      ? vendors[index]
      : `${vendors[index % vendors.length]} ${String(Math.floor(index / vendors.length) + 1).padStart(3, "0")}`,
  code: `V${String(index + 1).padStart(5, "0")}`,
});

export const familyFor = (index, seed) => {
  const [type, interfaceName, formFactor, wattage] =
    types[(index + seed) % types.length];
  const maker = vendors[(index + seed) % vendors.length];
  const line = [
    "Arc",
    "Vector",
    "Flux",
    "Forge",
    "Summit",
    "Pulse",
    "Apex",
    "Nova",
  ][(index * 3 + seed) % 8];
  const model = `${String(400 + ((index * 37 + seed) % 9500)).padStart(4, "0")}`;
  return {
    name: `${maker} ${line} ${model} ${type}`,
    code: `BFG-${line.slice(0, 3).toUpperCase()}-${model}`,
    description: `${type} for ByteForge Components system builders, with ${interfaceName} connectivity and validated integration specifications.`,
    type,
    interfaceName,
    formFactor,
    wattage,
  };
};

export const skuFor = (family, familyIndex, variantIndex, seed) => {
  const revision = String.fromCharCode(65 + (variantIndex % 4));
  const sku = `${family.code}-${String(variantIndex + 1).padStart(2, "0")}-${revision}`;
  const stock = (familyIndex * 17 + variantIndex * 13 + seed) % 71;
  return {
    name: `${family.name}, ${capacity[(familyIndex + variantIndex + seed) % capacity.length]}`,
    sku,
    description: `${family.description} Revision ${revision}; selected for ${stock === 0 ? "special order" : "regular"} availability.`,
    price: Number(
      (
        39 +
        ((familyIndex * 29 + variantIndex * 71 + seed) % 2400) +
        0.99
      ).toFixed(2),
    ),
    stock,
    availability:
      stock === 0 ? "special_order" : stock < 6 ? "low_stock" : "in_stock",
    capacity: capacity[(familyIndex + variantIndex + seed) % capacity.length],
  };
};

const pixel =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL9NwAAAABJRU5ErkJggg==";
const note = (title) =>
  `ByteForge Components\n${title}\nAll product names and specifications in this demonstration catalog are fictional.\n`;
const datasheet = (title) =>
  `%PDF-1.4\n1 0 obj<</Type/Catalog>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n% ${title}\n`;

export const familyValues = (family) => [
  ["name", family.name],
  ["family_code", family.code],
  ["description", family.description],
  ["product_type", family.type],
];

export const skuValues = (sku, family) => [
  ["name", sku.name],
  ["sku", sku.sku],
  ["description", sku.description],
  ["product_type", family.type],
  ["price", sku.price],
  ["stock_on_hand", sku.stock],
  ["availability", sku.availability],
  ["interface", family.interfaceName],
  ["capacity", sku.capacity],
  ["form_factor", family.formFactor],
  ["wattage", family.wattage],
];

export const relationshipFields = {
  categoryParent: "parent_category",
  familyCategory: "category",
  familyManufacturer: "manufacturer",
  skuFamily: "family",
  skuCategory: "category",
  skuManufacturer: "manufacturer",
  skuCompatible: "compatible_skus",
  skuFiles: "product_files",
};

export const assets = [
  ["arc-gpu", "image/png", pixel],
  ["vector-board", "image/png", pixel],
  ["flux-storage", "image/png", pixel],
  ["forge-memory", "image/png", pixel],
  ["voltarra-power", "image/png", pixel],
  ["ridgepoint-cooling", "image/png", pixel],
  ["installation-guide", "text/plain", note("Installation guide")],
  ["warranty-notes", "text/plain", note("Warranty notes")],
  ["compatibility-notes", "text/plain", note("Compatibility notes")],
  ["firmware-readme", "text/plain", note("Firmware readme")],
  ["arc-datasheet", "application/pdf", datasheet("Arc data sheet")],
  ["vector-datasheet", "application/pdf", datasheet("Vector data sheet")],
].map(([name, type, body]) => ({
  name: `byteforge-${name}.${type === "image/png" ? "png" : type === "application/pdf" ? "pdf" : "txt"}`,
  type,
  body,
}));
