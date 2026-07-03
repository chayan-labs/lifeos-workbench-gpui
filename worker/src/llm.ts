// Claude Haiku integration - issue #64 (docs/ARCHITECTURE.md §3.1: "light/
// medium brain", Haiku, cheap and always-on). Heavy reasoning/codegen never
// runs here - it's enqueued to the Mac (`jobs`), same boundary as every
// other cloud-side component in this repo.
import Anthropic from "@anthropic-ai/sdk";

const HAIKU_MODEL = "claude-haiku-4-5";

export async function askHaiku(apiKey: string, systemPrompt: string, userMessage: string): Promise<string> {
  const client = new Anthropic({ apiKey });
  const response = await client.messages.create({
    model: HAIKU_MODEL,
    max_tokens: 512,
    system: systemPrompt,
    messages: [{ role: "user", content: userMessage }],
  });

  const textBlock = response.content.find((block) => block.type === "text");
  return textBlock?.type === "text" ? textBlock.text : "";
}
