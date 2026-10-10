#!/usr/bin/env node

import { existsSync } from "node:fs";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import process from "node:process";
import * as pcComponents from "./generator/industries/pc-components.mjs";

const industries = { "pc-components": pcComponents };
const rootContextId = "00000000-0000-4000-8000-000000000001";
const startedAt = Date.now();

const usage = `Usage: node examples/generate.mjs [options]

Options:
  --industry <name>       Industry pack (default: pc-components)
  --size <profile>        micro, small, medium, or large (default: small)
  --seed <number>         Deterministic seed (default: 214)
  --concurrency <number>  Concurrent family work items (default: 1)
  --resume                Resume the matching local checkpoint
  --status                Print matching checkpoint status without writing
  --dry-run               Print the work plan without contacting the API
  --no-files              Skip the fixed demo asset bundle
  --no-publish            Leave generated records unpublished
  --progress <tty|json>   Progress format (default: tty when interactive)
  --checkpoint <path>     Override checkpoint location
  --help                  Show this help

ATTRICAT_TOKEN is required for writes. ATTRICAT_SERVER defaults to http://127.0.0.1:3000.
The generator refuses non-local targets unless ALLOW_NON_LOCAL_GENERATOR_TARGET=1.`;

const parseArgs = (argv) => {
  const options = {
    industry: process.env.CATALOG_INDUSTRY ?? "pc-components",
    size: process.env.CATALOG_SIZE ?? "small",
    seed: Number(process.env.CATALOG_SEED ?? 214),
    concurrency: Number(process.env.CATALOG_CONCURRENCY ?? 1),
    resume: false,
    status: false,
    dryRun: false,
    files: process.env.CATALOG_INCLUDE_FILES !== "0",
    publish: process.env.CATALOG_PUBLISH !== "0",
    progress: process.stdout.isTTY ? "tty" : "json",
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--help") return { help: true };
    if (arg === "--resume") options.resume = true;
    else if (arg === "--status") options.status = true;
    else if (arg === "--dry-run") options.dryRun = true;
    else if (arg === "--no-files") options.files = false;
    else if (arg === "--no-publish") options.publish = false;
    else if (
      [
        "--industry",
        "--size",
        "--seed",
        "--concurrency",
        "--progress",
        "--checkpoint",
      ].includes(arg)
    ) {
      const value = argv[++index];
      if (!value || value.startsWith("--"))
        throw new Error(`${arg} requires a value`);
      const key = arg
        .slice(2)
        .replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      options[key] =
        key === "seed" || key === "concurrency" ? Number(value) : value;
    } else throw new Error(`Unknown option: ${arg}`);
  }
  if (!Number.isInteger(options.seed) || options.seed < 0)
    throw new Error("--seed must be a non-negative integer");
  if (
    !Number.isInteger(options.concurrency) ||
    options.concurrency < 1 ||
    options.concurrency > 32
  )
    throw new Error("--concurrency must be an integer from 1 to 32");
  if (!["tty", "json"].includes(options.progress))
    throw new Error("--progress must be tty or json");
  return options;
};

const assertLocalTarget = (server) => {
  const url = new URL(server);
  const localHosts = new Set(["127.0.0.1", "localhost", "::1"]);
  if (
    !localHosts.has(url.hostname) &&
    process.env.ALLOW_NON_LOCAL_GENERATOR_TARGET !== "1"
  ) {
    throw new Error(
      `Refusing non-local generator target ${url.origin}; set ALLOW_NON_LOCAL_GENERATOR_TARGET=1 to override deliberately.`,
    );
  }
  return url.origin.replace(/\/$/, "");
};

const scalar = (attribute_code, value, context_id = rootContextId) => ({
  kind: "scalar",
  attribute_code,
  context_id,
  value,
});
const now = () => new Date().toISOString();
const pad = (value, width = 6) => String(value).padStart(width, "0");

class Progress {
  constructor(options, plan) {
    this.options = options;
    this.plan = plan;
    this.completedRecords = 0;
    this.completedRequests = 0;
    this.failedRequests = 0;
    this.retriedRequests = 0;
    this.phase = "preflight";
    this.lastRender = 0;
    this.lastSample = { at: Date.now(), records: 0 };
    this.rate = 0;
  }

  event(event, extra = {}) {
    const elapsedSeconds = Math.max((Date.now() - startedAt) / 1000, 0.001);
    const remaining = Math.max(this.plan.records - this.completedRecords, 0);
    const etaSeconds = this.rate > 0 ? Math.ceil(remaining / this.rate) : null;
    const payload = {
      event,
      phase: this.phase,
      records: {
        completed: this.completedRecords,
        total: this.plan.records,
      },
      requests: {
        completed: this.completedRequests,
        failed: this.failedRequests,
        retried: this.retriedRequests,
      },
      elapsed_seconds: Number(elapsedSeconds.toFixed(1)),
      records_per_second: Number(this.rate.toFixed(2)),
      eta_seconds: etaSeconds,
      ...extra,
    };
    if (this.options.progress === "json")
      process.stdout.write(`${JSON.stringify(payload)}\n`);
    else if (event === "progress" || event === "complete") {
      const percent = (
        (this.completedRecords / this.plan.records) *
        100
      ).toFixed(1);
      const eta =
        etaSeconds === null ? "calculating" : `${Math.ceil(etaSeconds / 60)}m`;
      process.stdout.write(
        `\r[${this.phase}] ${pad(this.completedRecords, 7)}/${this.plan.records} records (${percent}%) · ${this.rate.toFixed(1)}/s · ETA ${eta} · retries ${this.retriedRequests}`,
      );
      if (event === "complete") process.stdout.write("\n");
    } else process.stdout.write(`[${this.phase}] ${event}\n`);
  }

  tick({ records = 0, requests = 0, retries = 0 } = {}) {
    this.completedRecords += records;
    this.completedRequests += requests;
    this.retriedRequests += retries;
    const sampleNow = Date.now();
    const deltaSeconds = (sampleNow - this.lastSample.at) / 1000;
    if (deltaSeconds >= 2) {
      const sampleRate =
        (this.completedRecords - this.lastSample.records) / deltaSeconds;
      this.rate =
        this.rate === 0 ? sampleRate : this.rate * 0.7 + sampleRate * 0.3;
      this.lastSample = { at: sampleNow, records: this.completedRecords };
    }
    if (sampleNow - this.lastRender >= 1000) {
      this.lastRender = sampleNow;
      this.event("progress");
    }
  }
}

const atomicJson = async (path, value) => {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.tmp`;
  await writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`);
  await rename(temporary, path);
};

const checkpointPathFor = (options, pack) =>
  options.checkpoint
    ? resolve(options.checkpoint)
    : resolve(
        ".catalog-generator",
        `${options.industry}-v${pack.schemaVersion}-${options.size}-seed${options.seed}.json`,
      );

const createClient = (server, token, progress) => {
  const request = async (path, options = {}, attempt = 0) => {
    const headers = { authorization: `Bearer ${token}`, ...options.headers };
    if (options.body && !(options.body instanceof FormData))
      headers["content-type"] = "application/json";
    try {
      const response = await fetch(`${server}${path}`, { ...options, headers });
      const body = await response.json().catch(() => null);
      progress.tick({ requests: 1 });
      if (response.ok) return body;
      const retryable = response.status === 429 || response.status >= 500;
      if (retryable && attempt < 5) {
        progress.retriedRequests += 1;
        progress.event("retry", {
          method: options.method ?? "GET",
          path,
          status: response.status,
          attempt: attempt + 1,
        });
        await new Promise((done) => setTimeout(done, 200 * 2 ** attempt));
        return request(path, options, attempt + 1);
      }
      const message = body?.error?.message ?? response.statusText;
      throw new Error(
        `${options.method ?? "GET"} ${path} failed (${response.status}): ${message}`,
      );
    } catch (error) {
      if (attempt < 5 && error instanceof TypeError) {
        progress.retriedRequests += 1;
        progress.event("retry", {
          method: options.method ?? "GET",
          path,
          reason: error.message,
          attempt: attempt + 1,
        });
        await new Promise((done) => setTimeout(done, 200 * 2 ** attempt));
        return request(path, options, attempt + 1);
      }
      throw error;
    }
  };
  return {
    request,
    createRecord: (blueprint, values, metadata, tag) =>
      request("/v1/records", {
        method: "POST",
        body: JSON.stringify({
          blueprint: { code: blueprint },
          values,
          system_tags: [tag],
          system_metadata: metadata,
        }),
      }),
    replaceRelationships: (id, relationships) =>
      request(`/records/${id}/relationships/replace`, {
        method: "POST",
        body: JSON.stringify({ relationships }),
      }),
    publishAll: (id) =>
      request(`/v1/records/${id}/publications/publish-all`, {
        method: "POST",
      }),
    enablePublicationChannel: (contextId) =>
      request(`/publication-channels/${contextId}`, {
        method: "PUT",
        body: JSON.stringify({ enabled: true }),
      }),
  };
};

const ensureBlueprint = async (client, code, source) => {
  const publish = (blueprint) =>
    client.request(
      `/blueprints/${blueprint.blueprint.id}/versions/${blueprint.blueprint.version}/publish`,
      { method: "POST" },
    );
  try {
    const current = await client.request(
      `/blueprints/by-code/${code}?include_drafts=true`,
    );
    if (current.blueprint.definition === source)
      return current.blueprint.status === "published"
        ? current
        : publish(current);
    return publish(
      await client.request(`/blueprints/${current.blueprint.id}/versions`, {
        method: "POST",
        body: JSON.stringify({ definition: source }),
      }),
    );
  } catch (error) {
    if (!error.message.includes("(404)")) throw error;
    return publish(
      await client.request("/blueprints", {
        method: "POST",
        body: JSON.stringify({ definition: source }),
      }),
    );
  }
};

const ensureContext = async (client, code, data, parentId) => {
  try {
    return await client.request(`/contexts/${code}`);
  } catch (error) {
    if (!error.message.includes("(404)")) throw error;
    return client.request("/contexts", {
      method: "POST",
      body: JSON.stringify({ code, data, parent_id: parentId }),
    });
  }
};

const countPlan = (profile, includeFiles, pack) => {
  const classifications = Object.values(pack.classificationValues).reduce(
    (total, values) => total + values.length,
    0,
  );
  return {
    records:
      Object.values(profile).reduce((total, value) => total + value, 0) +
      classifications,
    requests:
      classifications +
      profile.categories * 2 +
      profile.manufacturers +
      profile.families * 2 +
      profile.skus * 2 +
      (includeFiles ? pack.assets.length * 2 : 0),
  };
};

const readCheckpoint = async (path) => JSON.parse(await readFile(path, "utf8"));

const runPool = async (items, concurrency, work) => {
  let cursor = 0;
  const worker = async () => {
    while (cursor < items.length) {
      const item = items[cursor++];
      await work(item);
    }
  };
  await Promise.all(
    Array.from({ length: Math.min(concurrency, items.length) }, worker),
  );
};

const fileBody = (asset) =>
  asset.type === "image/png"
    ? Uint8Array.from(Buffer.from(asset.body, "base64"))
    : asset.body;

const run = async () => {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) return process.stdout.write(`${usage}\n`);
  const pack = industries[options.industry];
  if (!pack)
    throw new Error(
      `Unknown industry ${options.industry}; available: ${Object.keys(industries).join(", ")}`,
    );
  const profile = pack.profiles[options.size];
  if (!profile)
    throw new Error(
      `Unknown size ${options.size}; available: ${Object.keys(pack.profiles).join(", ")}`,
    );
  const plan = countPlan(profile, options.files, pack);
  const checkpointPath = checkpointPathFor(options, pack);
  const datasetId = `${options.industry}:v${pack.schemaVersion}:${options.size}:seed${options.seed}`;
  const output = {
    industry: options.industry,
    size: options.size,
    schema_version: pack.schemaVersion,
    seed: options.seed,
    dataset_id: datasetId,
    plan,
    checkpoint: checkpointPath,
  };
  if (options.dryRun)
    return process.stdout.write(`${JSON.stringify(output, null, 2)}\n`);
  if (options.status) {
    if (!existsSync(checkpointPath))
      return process.stdout.write(
        `${JSON.stringify({ ...output, status: "not_started" }, null, 2)}\n`,
      );
    return process.stdout.write(
      `${JSON.stringify({ ...output, ...(await readCheckpoint(checkpointPath)) }, null, 2)}\n`,
    );
  }
  const server = assertLocalTarget(
    process.env.ATTRICAT_SERVER ?? "http://127.0.0.1:3000",
  );
  const token = `${process.env.ATTRICAT_TOKEN ?? ""}`.trim();
  if (!token)
    throw new Error(
      "Set ATTRICAT_TOKEN to a personal access token before running the generator.",
    );
  if (existsSync(checkpointPath) && !options.resume)
    throw new Error(
      `Checkpoint already exists at ${checkpointPath}; use --resume or choose a new --seed/checkpoint.`,
    );
  const progress = new Progress(options, plan);
  const client = createClient(server, token, progress);
  const publish = async (recordId) => {
    if (options.publish) await client.publishAll(recordId);
  };
  let checkpoint = existsSync(checkpointPath)
    ? await readCheckpoint(checkpointPath)
    : {
        dataset_id: datasetId,
        industry: options.industry,
        schema_version: pack.schemaVersion,
        size: options.size,
        seed: options.seed,
        started_at: now(),
        phase: "blueprints",
        manufacturer_ids: [],
        category_ids: [],
        classification_ids: {},
        family_cursor: 0,
        file_targets: [],
        counts: { records: 0 },
        files: options.files,
      };
  if (checkpoint.dataset_id !== datasetId)
    throw new Error(
      "Checkpoint does not match the selected industry, schema version, size, and seed.",
    );
  const defaultContext = await client.request("/contexts/default");
  await client.enablePublicationChannel(defaultContext.id);
  progress.completedRecords = checkpoint.counts.records;
  progress.lastSample.records = checkpoint.counts.records;
  const persist = async () => atomicJson(checkpointPath, checkpoint);
  const tag = `generator:${options.industry}:v${pack.schemaVersion}`;
  const metadata = (kind, ordinal) => ({
    generator_dataset: datasetId,
    generator_schema_version: pack.schemaVersion,
    generator_kind: kind,
    generator_ordinal: ordinal,
  });
  const definitions = pack.blueprints(
    `demo_${options.industry.replace(/-/g, "_")}_v${pack.schemaVersion}`,
  );
  const codes = Object.fromEntries(
    Object.keys(definitions).map((key) => [
      key,
      `demo_${options.industry.replace(/-/g, "_")}_v${pack.schemaVersion}_${key}`,
    ]),
  );
  const fields = pack.relationshipFields;

  let interrupted = false;
  const stop = () => {
    interrupted = true;
  };
  const stopIfRequested = async () => {
    if (!interrupted) return false;
    checkpoint.interrupted_at = now();
    checkpoint.benchmark_ready = false;
    await persist();
    progress.event("interrupted", {
      checkpoint: checkpointPath,
      resumable: true,
    });
    process.exitCode = 130;
    return true;
  };
  process.once("SIGINT", stop);
  try {
    progress.phase = "preflight";
    progress.event("starting", { dataset_id: datasetId, server });
    await client.request("/health");
    if (checkpoint.phase === "blueprints") {
      progress.phase = "blueprints";
      for (const [key, definition] of Object.entries(definitions))
        await ensureBlueprint(client, codes[key], definition);
      checkpoint.phase = "contexts";
      await persist();
      if (await stopIfRequested()) return;
    }
    if (checkpoint.phase === "contexts") {
      progress.phase = "contexts";
      const root = await ensureContext(
        client,
        `demo-${options.industry}-v${pack.schemaVersion}`,
        { industry: options.industry, dataset: datasetId },
        rootContextId,
      );
      await ensureContext(
        client,
        `demo-${options.industry}-v${pack.schemaVersion}-web`,
        { channel: "web", dataset: datasetId },
        root.id,
      );
      checkpoint.phase = "classifications";
      await persist();
      if (await stopIfRequested()) return;
    }
    if (checkpoint.phase === "classifications") {
      progress.phase = "classifications";
      for (const [kind, values] of Object.entries(pack.classificationValues)) {
        const ids = (checkpoint.classification_ids[kind] ??= []);
        for (let index = ids.length; index < values.length; index += 1) {
          const record = await client.createRecord(
            codes[kind],
            [
              scalar("name", values[index]),
              scalar("code", `pc-${kind}-${index + 1}`),
            ],
            metadata(kind, index),
            tag,
          );
          await publish(record.id);
          ids.push(record.id);
          checkpoint.counts.records += 1;
          progress.tick({ records: 1 });
          await persist();
          if (await stopIfRequested()) return;
        }
      }
      checkpoint.phase = "manufacturers";
      await persist();
    }
    if (checkpoint.phase === "manufacturers") {
      progress.phase = "manufacturers";
      for (
        let index = checkpoint.manufacturer_ids.length;
        index < profile.manufacturers;
        index += 1
      ) {
        const manufacturer = pack.manufacturerFor(index);
        const record = await client.createRecord(
          codes.manufacturer,
          [
            scalar("name", manufacturer.name),
            scalar("vendor_code", manufacturer.code),
          ],
          metadata("manufacturer", index),
          tag,
        );
        await publish(record.id);
        checkpoint.manufacturer_ids.push(record.id);
        checkpoint.counts.records += 1;
        progress.tick({ records: 1 });
        await persist();
        if (await stopIfRequested()) return;
      }
      checkpoint.phase = "categories";
      await persist();
    }
    if (checkpoint.phase === "categories") {
      progress.phase = "categories";
      for (
        let index = checkpoint.category_ids.length;
        index < profile.categories;
        index += 1
      ) {
        const category = pack.categoryFor(index);
        const record = await client.createRecord(
          codes.category,
          [scalar("name", category.name), scalar("slug", category.slug)],
          metadata("category", index),
          tag,
        );
        if (category.parentIndex !== null)
          await client.replaceRelationships(record.id, [
            {
              attribute_code: fields.categoryParent,
              context_id: rootContextId,
              target_record_ids: [
                checkpoint.category_ids[category.parentIndex],
              ],
            },
          ]);
        await publish(record.id);
        checkpoint.category_ids.push(record.id);
        checkpoint.counts.records += 1;
        progress.tick({ records: 1 });
        await persist();
        if (await stopIfRequested()) return;
      }
      checkpoint.phase = "families";
      await persist();
    }
    if (checkpoint.phase === "families") {
      progress.phase = "families";
      const familiesPerRun = Math.min(options.concurrency, 32);
      while (checkpoint.family_cursor < profile.families) {
        const start = checkpoint.family_cursor;
        const end = Math.min(start + familiesPerRun, profile.families);
        await runPool(
          Array.from({ length: end - start }, (_, offset) => start + offset),
          options.concurrency,
          async (index) => {
            const family = pack.familyFor(index, options.seed);
            const categoryId =
              checkpoint.category_ids[index % checkpoint.category_ids.length];
            const manufacturerId =
              checkpoint.manufacturer_ids[
                index % checkpoint.manufacturer_ids.length
              ];
            const familyRecord = await client.createRecord(
              codes.family,
              pack
                .familyValues(family)
                .map(([code, value]) => scalar(code, value)),
              metadata("family", index),
              tag,
            );
            const classificationId = (kind, value) => {
              const valueIndex = pack.classificationValues[kind].indexOf(value);
              if (valueIndex < 0)
                throw new Error(`Missing ${kind} classification for ${value}`);
              return checkpoint.classification_ids[kind][valueIndex];
            };
            await client.replaceRelationships(familyRecord.id, [
              {
                attribute_code: fields.familyProductType,
                context_id: rootContextId,
                target_record_ids: [
                  classificationId("product_type", family.type),
                ],
              },
              {
                attribute_code: fields.familyInterfaceStandard,
                context_id: rootContextId,
                target_record_ids: [
                  classificationId("interface_standard", family.interfaceName),
                ],
              },
              {
                attribute_code: fields.familyFormFactor,
                context_id: rootContextId,
                target_record_ids: [
                  classificationId("form_factor", family.formFactor),
                ],
              },
              {
                attribute_code: fields.familyCategory,
                context_id: rootContextId,
                target_record_ids: [categoryId],
              },
              {
                attribute_code: fields.familyManufacturer,
                context_id: rootContextId,
                target_record_ids: [manufacturerId],
              },
            ]);
            await publish(familyRecord.id);
            const skuRecords = [];
            for (let variant = 0; variant < 4; variant += 1) {
              const sku = pack.skuFor(family, index, variant, options.seed);
              const record = await client.createRecord(
                codes.sku,
                pack
                  .skuValues(sku, family)
                  .map(([code, value]) => scalar(code, value)),
                metadata("sku", index * 4 + variant),
                tag,
              );
              skuRecords.push(record);
            }
            // Relationship replacement mutates current-value history. Keep the four
            // variant writes serial so a normal seed does not manufacture avoidable
            // transient database conflicts and retries.
            for (const [variant, record] of skuRecords.entries()) {
              await client.replaceRelationships(record.id, [
                {
                  attribute_code: fields.skuFamily,
                  context_id: rootContextId,
                  target_record_ids: [familyRecord.id],
                },
                {
                  attribute_code: fields.skuCompatible,
                  context_id: rootContextId,
                  target_record_ids: [
                    skuRecords[(variant + 1) % skuRecords.length].id,
                  ],
                },
              ]);
            }
            for (const record of skuRecords) await publish(record.id);
            if (index < pack.assets.length)
              checkpoint.file_targets[index] = skuRecords[0].id;
            progress.tick({ records: 5 });
          },
        );
        checkpoint.family_cursor = end;
        checkpoint.counts.records =
          Object.values(pack.classificationValues).reduce(
            (total, values) => total + values.length,
            0,
          ) +
          profile.manufacturers +
          profile.categories +
          end * 5;
        await persist();
        if (await stopIfRequested()) return;
      }
      checkpoint.phase = options.files ? "files" : "verify";
      await persist();
    }
    if (checkpoint.phase === "files") {
      progress.phase = "files";
      for (
        let index = checkpoint.files_completed ?? 0;
        index < pack.assets.length;
        index += 1
      ) {
        const asset = pack.assets[index];
        const form = new FormData();
        form.append(
          "file",
          new Blob([fileBody(asset)], { type: asset.type }),
          asset.name,
        );
        const attributeCode = asset.type.startsWith("image/")
          ? fields.skuMainPhoto
          : fields.skuFiles;
        const uploaded = await client.request(
          `/records/${checkpoint.file_targets[index]}/file-attributes/${attributeCode}/uploads`,
          { method: "POST", body: form },
        );
        const file = uploaded.files?.[0];
        if (!file?.id)
          throw new Error(
            `File upload response for ${asset.name} did not contain a file ID`,
          );
        let metadataResponse = file;
        for (
          let attempt = 0;
          attempt < 30 && metadataResponse.status !== "ready";
          attempt += 1
        ) {
          await new Promise((done) => setTimeout(done, 1000));
          try {
            metadataResponse = await client.request(`/files/${file.id}`);
          } catch (error) {
            if (error.message.includes("(403)"))
              throw new Error(
                "File verification requires records.read for the uploaded record; recreate ATTRICAT_TOKEN with records.read and records.write, or rerun a clean dataset with --no-files.",
              );
            throw error;
          }
        }
        if (metadataResponse.status !== "ready")
          throw new Error(`File ${asset.name} did not finish processing`);
        await publish(checkpoint.file_targets[index]);
        checkpoint.files_completed = index + 1;
        await persist();
        if (await stopIfRequested()) return;
      }
      checkpoint.phase = "verify";
      await persist();
    }
    if (checkpoint.phase === "verify") {
      progress.phase = "verify";
      const sampleIds = [
        checkpoint.manufacturer_ids[0],
        checkpoint.category_ids.at(-1),
        checkpoint.file_targets[0],
      ].filter(Boolean);
      for (const id of sampleIds) await client.request(`/records/${id}`);
      checkpoint.phase = "complete";
      checkpoint.completed_at = now();
      checkpoint.benchmark_ready = true;
      await persist();
    }
    progress.event("complete", {
      checkpoint: checkpointPath,
      benchmark_ready: checkpoint.benchmark_ready,
    });
  } finally {
    process.removeListener("SIGINT", stop);
  }
};

run().catch((error) => {
  console.error(`Generator failed: ${error.message}`);
  process.exitCode = 1;
});
