// Used only by test-reset.mjs inside its disposable Compose deployment.
import assert from "node:assert/strict";
import { connect, readFixture, verify } from "./load.mjs";

const { fixture } = await readFixture();
const server = process.env.CATALOG_SERVER;
if (process.argv[2] === "mutate") {
  const client = await connect();
  try {
    const page = await client.request("/v1/entities/search", "POST", {
      blueprint: { code: fixture.product_blueprint },
      page: { size: 100 },
    });
    const context = await client.request("/contexts/default");
    await client.request(`/entities/${page.items[0].id}/values`, "POST", {
      values: [
        {
          kind: "scalar",
          attribute_code: "name",
          context_id: context.id,
          value: "Visitor changed this product",
        },
      ],
    });
    await client.request(`/entities/${page.items[1].id}`, "DELETE");
    await client.request("/v1/entities", "POST", {
      blueprint: { code: fixture.product_blueprint },
      values: [
        {
          kind: "scalar",
          attribute_code: "name",
          context_id: context.id,
          value: "Visitor extra product",
        },
      ],
    });
    await assert.rejects(verify(client.request, fixture));
    const token = await client.request("/personal-access-tokens", "POST", {
      label: "Disposable reset test",
      permissions: ["blueprints.read"],
    });
    const session = await fetch(`${server}/auth/login`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        login_identifier: "default.local",
        email: "owner@example.com",
        password: "test",
      }),
    });
    assert.equal(session.status, 200);
    console.log(
      JSON.stringify({
        cookie: session.headers
          .getSetCookie()
          .map((c) => c.split(";")[0])
          .join("; "),
        token: token.secret,
        oldIds: page.items.map((i) => i.id),
      }),
    );
  } finally {
    await client.logout();
  }
} else if (process.argv[2] === "check-old-credentials") {
  const old = JSON.parse(process.env.OLD_STATE);
  const response = await fetch(`${server}/auth/session`, {
    headers: { cookie: old.cookie },
  });
  assert.equal(response.status, 401, "old session must be invalid");
  const tokenResponse = await fetch(`${server}/blueprints`, {
    headers: { authorization: `Bearer ${old.token}` },
  });
  assert.equal(tokenResponse.status, 401, "old PAT must be invalid");
  const client = await connect();
  try {
    await verify(client.request, fixture);
    const page = await client.request("/v1/entities/search", "POST", {
      blueprint: { code: fixture.product_blueprint },
      page: { size: 100 },
    });
    assert.ok(
      page.items.every((item) => !old.oldIds.includes(item.id)),
      "entities must be recreated, not patched in place",
    );
  } finally {
    await client.logout();
  }
  console.log("Old sessions, tokens and IDs discarded; fixture fully restored");
} else {
  throw new Error("unknown test-client mode");
}
