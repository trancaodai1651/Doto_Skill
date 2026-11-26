import test from "node:test";
import assert from "node:assert/strict";
import { discoverSkills, composeSkillPrompt } from "../server/skills.mjs";
import {
  getProvidersCatalog,
  buildInvocationPreview,
  resolveAntigravityModelSlug,
} from "../server/providers.mjs";
import { startServer } from "../server/index.mjs";

test("discoverSkills phát hiện đủ 6 skill cốt lõi và danh sách chủ đề giọng đọc", () => {
  const info = discoverSkills("E:\\_TRAN CAO DAI\\SKILL");
  const ids = info.skills.map((s) => s.id);
  for (const expected of [
    "stickman-video",
    "news-video",
    "crime-case-video",
    "crime-documentary",
    "product-ad-video",
    "capcut-livestream-edit",
  ]) {
    assert.ok(ids.includes(expected), `Thiếu skill ${expected}`);
  }
  assert.ok(info.topics.length >= 8, "Phải có danh sách chủ đề giọng đọc VieNeu");
});

test("composeSkillPrompt sinh đúng tiền tố /skill cho Claude & Antigravity và $skill cho Codex", () => {
  const claudeCmd = composeSkillPrompt({
    skillId: "news-video",
    providerId: "claude",
    brief: "Bản tin sáng nay",
    topic: "thoi-su",
    format: "9:16",
    renderStage: "draft",
  });
  assert.ok(claudeCmd.startsWith("/news-video "));

  const codexCmd = composeSkillPrompt({
    skillId: "stickman-video",
    providerId: "codex",
    brief: "Bản tin người que",
    stickMode: "ban-tin",
    character: "pro",
    format: "9:16",
    renderStage: "final",
  });
  assert.ok(codexCmd.startsWith("$stickman-video "));
  assert.ok(codexCmd.includes("--mode ban-tin"));

  const agyCmd = composeSkillPrompt({
    skillId: "crime-documentary",
    providerId: "antigravity",
    brief: "Phim tài liệu 3D",
    topic: "vu-an",
    format: "16:9",
  });
  assert.ok(agyCmd.startsWith("/crime-documentary "));
});

test("getProvidersCatalog & buildInvocationPreview ánh xạ chính xác Model và Effort riêng từng AI", async () => {
  const cat = await getProvidersCatalog({ force: true });
  assert.equal(cat.providers.length, 3);

  const claude = cat.providers.find((p) => p.id === "claude");
  const codex = cat.providers.find((p) => p.id === "codex");
  const agy = cat.providers.find((p) => p.id === "antigravity");

  assert.ok(claude.models.some((m) => m.id === "claude-opus-5-5" && m.efforts.includes("max")));
  assert.ok(codex.models.length >= 1 && codex.models[0].efforts.includes("high"));
  assert.ok(agy.models.some((m) => m.id === "gemini-4-neon"));

  // Kiểm tra ghép slug + effort của Antigravity
  assert.equal(resolveAntigravityModelSlug("gemini-4-neon", "xhigh"), "gemini-4-neon-xhigh");
  assert.equal(resolveAntigravityModelSlug("gemini-pro-orchestrator-neon", ""), "gemini-pro-orchestrator-neon");

  const pClaude = buildInvocationPreview({
    providerId: "claude",
    model: "claude-opus-5-5",
    effort: "xhigh",
    prompt: "/news-video test",
    skillRoot: "E:\\_TRAN CAO DAI\\SKILL",
  });
  assert.ok(pClaude.args.includes("--model") && pClaude.args.includes("claude-opus-5-5"));
  assert.ok(pClaude.args.includes("--effort") && pClaude.args.includes("xhigh"));

  const pCodex = buildInvocationPreview({
    providerId: "codex",
    model: "gpt-6.2-sol",
    effort: "ultra",
    prompt: "$news-video test",
    skillRoot: "E:\\_TRAN CAO DAI\\SKILL",
  });
  assert.ok(pCodex.args.includes("-m") && pCodex.args.includes("gpt-6.2-sol"));
  assert.ok(pCodex.args.includes('model_reasoning_effort="ultra"'));

  const pAgy = buildInvocationPreview({
    providerId: "antigravity",
    model: "gemini-4-carbon",
    effort: "high",
    prompt: "/news-video test",
    skillRoot: "E:\\_TRAN CAO DAI\\SKILL",
  });
  assert.ok(pAgy.args.includes("--model") && pAgy.args.includes("gemini-4-carbon-high"));
  assert.ok(pAgy.args.includes("--effort") && pAgy.args.includes("high"));
});

test("HTTP API /api/doctor & /api/bootstrap phản hồi đầy đủ", async () => {
  const { server, url } = await startServer({ port: 0, host: "127.0.0.1" });
  const addr = server.address();
  const baseUrl = `http://127.0.0.1:${addr.port}`;
  try {
    const doc = await (await fetch(`${baseUrl}/api/doctor`)).json();
    assert.equal(doc.ok, true);

    const boot = await (await fetch(`${baseUrl}/api/bootstrap`)).json();
    assert.equal(boot.providers.length, 3);
    assert.ok(Array.isArray(boot.skillsInfo.skills));
  } finally {
    server.close();
  }
});
