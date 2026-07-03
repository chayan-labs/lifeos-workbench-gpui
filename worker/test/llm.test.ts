import { afterEach, describe, expect, it, vi } from "vitest";
import { askHaiku } from "../src/llm.js";

// The Anthropic SDK sends requests via global fetch - stub it instead of
// hitting the real API, same principle as bot.test.ts's offline pattern.
describe("askHaiku", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("calls the Haiku model and returns the text block", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const body = JSON.parse((init?.body as string) ?? "{}");
      expect(body.model).toBe("claude-haiku-4-5");
      expect(body.system).toBe("You are terse.");
      expect(body.messages).toEqual([{ role: "user", content: "ping" }]);

      return new Response(
        JSON.stringify({
          id: "msg_1",
          type: "message",
          role: "assistant",
          model: "claude-haiku-4-5",
          content: [{ type: "text", text: "pong" }],
          stop_reason: "end_turn",
          usage: { input_tokens: 1, output_tokens: 1 },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    });
    vi.stubGlobal("fetch", fetchMock);

    const reply = await askHaiku("fake-key", "You are terse.", "ping");

    expect(reply).toBe("pong");
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
