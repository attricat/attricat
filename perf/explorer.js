import http from "k6/http";
import { check, sleep } from "k6";

const server = (__ENV.ATTRICAT_SERVER || "http://127.0.0.1:3000").replace(
  /\/$/,
  "",
);
const token = __ENV.ATTRICAT_TOKEN;
const blueprint = __ENV.ATTRICAT_BLUEPRINT || "demo_pc_components_v1_sku";
const contextId =
  __ENV.ATTRICAT_CONTEXT_ID || "00000000-0000-4000-8000-000000000001";
const profile = __ENV.PERF_PROFILE || "smoke";

if (!token) throw new Error("ATTRICAT_TOKEN is required");

const profiles = {
  smoke: [{ duration: "20s", target: 2 }],
  baseline: [
    { duration: "30s", target: 1 },
    { duration: "90s", target: 4 },
    { duration: "30s", target: 0 },
  ],
  load: [
    { duration: "30s", target: 4 },
    { duration: "2m", target: 16 },
    { duration: "30s", target: 0 },
  ],
};

export const options = {
  scenarios: {
    explorer: {
      executor: "ramping-vus",
      stages: profiles[profile] || profiles.smoke,
      gracefulRampDown: "10s",
    },
  },
  thresholds: {
    http_req_failed: ["rate<0.01"],
  },
};

const headers = {
  authorization: `Bearer ${token}`,
  "content-type": "application/json",
};
const post = (path, body) =>
  http.post(`${server}${path}`, JSON.stringify(body), { headers });
const search = (body) => post("/v1/records/search", body);
const page = { size: 25, cursor: null };

export default function () {
  const variant = __ITER % 4;
  let response;
  if (variant === 0) {
    response = search({
      blueprint: { code: blueprint },
      query: "Arc",
      sort: { field: "price", direction: "asc" },
      page,
    });
  } else if (variant === 1) {
    response = search({
      blueprint: { code: blueprint },
      sort: { field: "manufacturer.name", direction: "asc" },
      page,
    });
  } else if (variant === 2) {
    const first = search({
      blueprint: { code: blueprint },
      sort: { field: "sku", direction: "asc" },
      page,
    });
    check(first, {
      "first keyset page is successful": (result) => result.status === 200,
    });
    const cursor = first.json("next_cursor");
    response = cursor
      ? search({
          blueprint: { code: blueprint },
          sort: { field: "sku", direction: "asc" },
          page: { size: 25, cursor },
        })
      : first;
  } else {
    response = post("/v1/records/facets/relationship-tree/children", {
      blueprint: { code: blueprint },
      source_relationship_field: "category",
      hierarchy_field: "parent_category",
      context_id: contextId,
      parent_id: null,
    });
  }
  check(response, {
    "Explorer request is successful": (result) => result.status === 200,
    "Explorer emits aggregate timing": (result) =>
      Boolean(result.headers["Server-Timing"]),
  });
  sleep(0.2);
}
