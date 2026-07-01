import { describe, expect, it } from "vitest";
import worker from "../src/index.js";

const TEST_ENV = {
  BOT_TOKEN: "fake",
  TURSO_URL: "libsql://fake",
  TURSO_TOKEN: "fake",
  ANTHROPIC_API_KEY: "fake",
};

// Only the non-Telegram-facing routes are unit-testable without a network
// call to Telegram's getMe (webhookCallback initializes the bot lazily on
// first request) - the `/telegram` path is verified live post-deployment,
// same as every other manual-setup-gated integration in this repo.
describe("worker fetch handler", () => {
  it("returns 200 ok on the liveness route", async () => {
    const res = await worker.fetch(new Request("https://example.com/"), TEST_ENV);
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("ok");
  });

  it("returns 404 on unknown routes", async () => {
    const res = await worker.fetch(new Request("https://example.com/nope"), TEST_ENV);
    expect(res.status).toBe(404);
  });

  it("returns 404 for a GET on the telegram webhook path", async () => {
    const res = await worker.fetch(new Request("https://example.com/telegram"), TEST_ENV);
    expect(res.status).toBe(404);
  });
});

// issue #71: `scheduled` no-ops without DIGEST_CHAT_ID - unit-testable since
// that path never touches the DB or the network. The send-a-real-digest path
// needs a live TURSO_URL + Telegram, so it's verified post-deployment, same
// as `/telegram` above.
describe("worker scheduled handler", () => {
  it("does nothing when DIGEST_CHAT_ID is unset", async () => {
    await expect(worker.scheduled(undefined as never, TEST_ENV)).resolves.toBeUndefined();
  });
});
