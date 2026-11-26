// Quét và phân tích toàn bộ 6 Skill cốt lõi + các Skill mở rộng được Import (từ thư mục gốc hoặc thư mục lưu tuỳ chỉnh).
import fs from "node:fs";
import path from "node:path";

export const SKILL_PRESETS = [
  {
    id: "stickman-video",
    title: "Người Que Viral (Bản tin & Vụ án)",
    titleEn: "Viral Stickman (Daily News & Crime Cases)",
    badge: "Hot · Dễ viral",
    badgeEn: "Hot · Viral Ready",
    engine: "Remotion 4 + VieNeu-TTS + Whisper",
    color: "#f97316",
    defaultFormat: "9:16",
    formats: ["9:16", "16:9"],
    defaultTopic: "thoi-su",
    supportsCharacter: true,
    supportsStickMode: true,
    estimatedTime: "~5–7 phút",
    recommendedProvider: "claude",
    summary:
      "Kể bản tin hôm nay hoặc hồ sơ vụ án bằng nhân vật (pro / stick / chibi / pixel), kết hợp ảnh/clip báo chí thật, bong bóng thoại tự né icon, khẩu hình khớp giọng đọc.",
    summaryEn:
      "Tell breaking news or crime case files with animated characters (pro / stick / chibi / pixel), real press photos/clips, smart speech bubbles, and lip-synced Vietnamese TTS.",
    quickTemplates: [
      {
        label: "Bản tin hôm nay (3 tin nóng)",
        stickMode: "ban-tin",
        topic: "thoi-su",
        character: "pro",
        brief:
          "Làm bản tin hôm nay: chọn 3 tin đáng chú ý nhất trong ngày, mỗi tin đối chiếu ≥ 2 báo chính thống, ưu tiên ảnh/video thật của báo (ghi nguồn trên hình), nhân vật kiểu \"pro\" chỉ diễn phần khuyến cáo/mô phỏng.",
      },
      {
        label: "Hồ sơ vụ án người que ẩn danh",
        stickMode: "ho-so-vu-an",
        topic: "vu-an",
        character: "pro",
        brief:
          "Làm hồ sơ vụ án 60 giây đúng tư cách tố tụng, đối chiếu ≥ 2 báo chính thống, che mặt toàn bộ ảnh bị can (blur-faces.mjs), dùng nhân vật kiểu \"pro\" diễn lại diễn biến không có hình.",
      },
      {
        label: "Tin công nghệ kiểu Pixel 8-bit",
        stickMode: "ban-tin",
        topic: "cong-nghe",
        character: "pixel",
        brief:
          "Làm bản tin công nghệ 60 giây với giọng trẻ nhanh, nhân vật kiểu \"pixel\" giải thích trực quan, có biểu đồ/số liệu và ghi nguồn rõ trên hình.",
      },
    ],
  },
  {
    id: "news-video",
    title: "Bản Tin Hằng Ngày Đa Cảnh",
    titleEn: "Multi-Scene Daily News Broadcast",
    badge: "Tin tức 60s",
    badgeEn: "60s News",
    engine: "Remotion 4 + HyperFrames",
    color: "#ef4444",
    defaultFormat: "9:16",
    formats: ["9:16", "16:9", "1:1"],
    defaultTopic: "thoi-su",
    supportsCharacter: true,
    supportsStickMode: false,
    estimatedTime: "~5 phút",
    recommendedProvider: "claude",
    summary:
      "Tự động thu tin RSS báo chính thống → đối chiếu FACTS.md → kịch bản hook 3s đầu, biểu đồ, số liệu, thẻ MXH, cảnh bàn tin 3D onetake và phụ đề karaoke.",
    summaryEn:
      "Auto-fetch verified RSS news → cross-check FACTS.md → 3s hook script, animated charts, stats, social cards, 3D one-take desk camera, and word-level karaoke captions.",
    quickTemplates: [
      {
        label: "Điểm nóng 24h (Thời sự)",
        topic: "thoi-su",
        character: "pro",
        brief:
          "Quét tin mới hôm nay, chọn 3 tin thời sự dân sinh quan trọng nhất, đối chiếu ≥ 2 nguồn báo chính thống mỗi tin, dựng bản tin 60 giây có hook 3s đầu và số liệu nổi bật.",
      },
      {
        label: "Bản tin Tài chính / Giá vàng",
        topic: "tai-chinh",
        character: "pro",
        brief:
          "Làm bản tin tài chính 60 giây tổng hợp diễn biến giá vàng, tỷ giá và chứng khoán hôm nay, có cảnh biểu đồ (chart) và con số đếm lên (stat), đối chiếu ≥ 2 báo.",
      },
      {
        label: "Bản tin có cảnh bàn 3D One-Take",
        topic: "cong-nghe",
        character: "pixel",
        brief:
          "Làm bản tin công nghệ 60 giây có 1 cảnh onetake bay trên bàn tin tức 3D và 1 cảnh minh hoạ tự vẽ, nguồn hiện rõ trên hình.",
      },
    ],
  },
  {
    id: "crime-case-video",
    title: "Hồ Sơ Vụ Án & Cảnh Báo Lừa Đảo",
    titleEn: "Crime Dossier & Scam Alert Video",
    badge: "Pháp luật",
    badgeEn: "Legal & Investigation",
    engine: "Remotion 4 + OpenCV YuNet",
    color: "#dc2626",
    defaultFormat: "9:16",
    formats: ["9:16", "16:9"],
    defaultTopic: "vu-an",
    supportsCharacter: true,
    supportsStickMode: false,
    estimatedTime: "~6 phút",
    recommendedProvider: "claude",
    summary:
      "Dựng hồ sơ điều tra đa cảnh nhịp nhanh: đúng tư cách tố tụng (nghi phạm/bị can/bị cáo), tự động khảm che mặt (blur-faces.mjs), sơ đồ dòng tiền, bằng chứng chỉ đỏ và cảnh báo phòng ngừa.",
    summaryEn:
      "Fast-paced investigative dossier: strict legal terminology, mandatory OpenCV YuNet face blurring, money-flow diagrams, red-string evidence boards, and public warnings.",
    quickTemplates: [
      {
        label: "Giải mã vụ khởi tố / xét xử mới",
        topic: "vu-an",
        character: "pro",
        brief:
          "Thu thập vụ án mới đáng chú ý trên báo chính thống, đối chiếu ≥ 2 nguồn, gọi đúng tư cách tố tụng, bắt buộc chạy blur-faces.mjs che mặt mọi ảnh nghi phạm/bị can, có sơ đồ dòng tiền và cảnh báo.",
      },
      {
        label: "Cảnh báo thủ đoạn lừa đảo mạng",
        topic: "lua-dao",
        character: "pro",
        brief:
          "Làm video 60 giây giải mã thủ đoạn lừa đảo công nghệ cao mới nhất theo khuyến cáo Công an, có sơ đồ đường dây, người que ẩn danh mô phỏng tình huống và 3 dấu hiệu nhận biết cho người dân.",
      },
      {
        label: "Trọng án (Giọng trầm, không máu me)",
        topic: "an-mang",
        character: "stick",
        brief:
          "Làm hồ sơ vụ việc với meta.topic \"an-mang\", giọng trầm nghiêm cẩn, chỉ tái hiện phong toả/điều tra/xử lý pháp luật, tuyệt đối không mô tả bạo lực, không nêu tên nạn nhân, che mặt toàn bộ ảnh.",
      },
    ],
  },
  {
    id: "crime-documentary",
    title: "Phim Tài Liệu \"Một Cú Máy\" 3D",
    titleEn: "3D One-Take Crime Documentary",
    badge: "3D + Vũ trụ WebGL",
    badgeEn: "3D + WebGL Space",
    engine: "HyperFrames + Three.js",
    color: "#eab308",
    defaultFormat: "16:9",
    formats: ["16:9"],
    defaultTopic: "vu-an",
    supportsCharacter: false,
    supportsStickMode: false,
    estimatedTime: "Nháp ~5p · Bản giao ~20p",
    recommendedProvider: "claude",
    summary:
      "Một camera duy nhất bay liên tục không cắt qua mặt bàn điều tra 3D (hồ sơ dưới đèn pin, cọc tiền, báo cắt dán), phóng ra vũ trụ Three.js so quy mô số tiền khổng lồ với motion blur thật.",
    summaryEn:
      "Single continuous camera flight over a 3D investigation desk (flashlight dossiers, cash stacks, clippings) zooming into Three.js space for massive scale comparisons.",
    quickTemplates: [
      {
        label: "Phim tài liệu đại án tài chính",
        topic: "vu-an",
        brief:
          "Làm phim tài liệu một cú máy 45–60 giây (16:9): viết data/script.json → tạo giọng VieNeu bằng voice-track.mjs (chủ đề vu-an) → che mặt mọi ảnh bằng blur-faces.mjs --dir assets/photos → đồng bộ nhịp words.mjs sync-beats → quy đổi quy mô tiền bằng convert.mjs (bay ra vũ trụ nếu số lớn) → kiểm hyperframes check ok:true.",
      },
    ],
  },
  {
    id: "product-ad-video",
    title: "Quảng Cáo Sản Phẩm Chuyển Đổi Cao",
    titleEn: "High-Converting Product Ad Video",
    badge: "PAS · AIDA · UGC",
    badgeEn: "PAS · AIDA · UGC",
    engine: "Remotion 4 + HyperFrames",
    color: "#a855f7",
    defaultFormat: "9:16",
    formats: ["9:16", "1:1", "16:9"],
    defaultTopic: "quang-cao",
    supportsCharacter: true,
    supportsStickMode: false,
    estimatedTime: "~5 phút",
    recommendedProvider: "claude",
    summary:
      "Bóc tách tự động từ Link sản phẩm hoặc thư mục Video/Ảnh thật (kể cả ảnh từ App Photos điện thoại), dựng kịch bản chuyển đổi PAS / AIDA / UGC, kiểm chuẩn Luật Quảng cáo.",
    summaryEn:
      "Auto-extract from product URLs or real photos/clips from mobile Photos app, craft PAS / AIDA / UGC conversion scripts with 3D studio camera & ad-compliance lint.",
    quickTemplates: [
      {
        label: "Quảng cáo 30s từ Link / Ảnh sản phẩm",
        topic: "quang-cao",
        character: "chibi",
        brief:
          "Làm video quảng cáo 30 giây theo khung PAS từ link/thông tin/ảnh sản phẩm đính kèm: có hook 3 giây dừng ngón tay, 1 cảnh onetake surface \"studio\" bay qua tem giá & thông số chính, kiểm tra lint-ad.mjs.",
      },
    ],
  },
  {
    id: "capcut-livestream-edit",
    title: "Biên Tập Livestream CapCut PC + B-Roll",
    titleEn: "CapCut PC Livestream Editor + B-Roll",
    badge: "CapCut Draft",
    badgeEn: "CapCut Draft",
    engine: "CapCut JSON + Remotion 4",
    color: "#14b8a6",
    defaultFormat: "16:9",
    formats: ["16:9", "9:16"],
    defaultTopic: "cong-nghe",
    supportsCharacter: false,
    supportsStickMode: false,
    estimatedTime: "Tuỳ độ dài video gốc",
    recommendedProvider: "codex",
    summary:
      "Tự động cắt khoảng lặng/lặp lời trong video dạy học hoặc livestream dài, sinh dự án nháp CapCut PC kèm B-roll đồ hoạ trắng-cam đồng bộ lời giảng.",
    summaryEn:
      "Automatically trim silences and filler words in long lectures or livestreams, generating a CapCut PC draft with synchronized motion-graphic B-roll.",
    quickTemplates: [
      {
        label: "Cắt gọt livestream + chèn B-roll",
        topic: "cong-nghe",
        brief:
          "Sao lưu bản nháp CapCut trước (capcut-draft.mjs backup), phân tích lời thoại, cắt bỏ đoạn ngập ngừng và sinh B-roll minh hoạ cho các ý chính.",
      },
    ],
  },
];

export const CORE_SKILL_IDS = SKILL_PRESETS.map((s) => s.id);

function parseFrontmatter(mdText) {
  const m = String(mdText || "").match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return {};
  const lines = m[1].split(/\r?\n/);
  const out = {};
  let currentKey = null;
  for (const line of lines) {
    const kv = line.match(/^([a-zA-Z0-9_-]+):\s*(.*)$/);
    if (kv) {
      currentKey = kv[1];
      const val = kv[2].trim();
      out[currentKey] = val === ">-" || val === "|" ? "" : val.replace(/^['"]|['"]$/g, "");
    } else if (currentKey && /^\s+/.test(line)) {
      out[currentKey] = `${out[currentKey] ? `${out[currentKey]} ` : ""}${line.trim()}`;
    }
  }
  return out;
}

export function loadTopicProfiles(skillRoot) {
  const file = path.join(skillRoot, "_shared", "scripts", "topic-profiles.json");
  try {
    const json = JSON.parse(fs.readFileSync(file, "utf8"));
    return Object.entries(json.topics || {}).map(([id, cfg]) => ({
      id,
      label: cfg.label || id,
      voiceName: cfg.voice?.name || "Tự động",
      voiceSpeed: cfg.voice?.speed || 1,
      voiceWhy: cfg.voice?.why || "",
      theme: cfg.theme || "default",
      tone: cfg.tone || "",
    }));
  } catch {
    return [
      { id: "thoi-su", label: "Thời sự / Bản tin hôm nay", voiceName: "Mai Anh", voiceSpeed: 1.08, theme: "breaking" },
      { id: "cong-nghe", label: "Tin công nghệ", voiceName: "Hải Đăng", voiceSpeed: 1.1, theme: "tech" },
      { id: "tai-chinh", label: "Kinh tế / Tài chính", voiceName: "Minh Đức", voiceSpeed: 1.05, theme: "finance" },
      { id: "the-gioi", label: "Thế giới", voiceName: "Ngọc Huyền", voiceSpeed: 1.06, theme: "world" },
      { id: "giai-tri", label: "Giải trí / Đời sống", voiceName: "Trúc Ly", voiceSpeed: 1.06, theme: "pop" },
      { id: "the-thao", label: "Thể thao", voiceName: "Adam bựa", voiceSpeed: 1.12, theme: "sport" },
      { id: "vu-an", label: "Vụ án / Hồ sơ pháp luật", voiceName: "Thiện Minh", voiceSpeed: 1.02, theme: "case" },
      { id: "an-mang", label: "Án mạng / Trọng án", voiceName: "Thiền Tâm Đức", voiceSpeed: 1.0, theme: "murder" },
      { id: "lua-dao", label: "Lừa đảo / Tội phạm mạng", voiceName: "Phạm Tuyên", voiceSpeed: 1.04, theme: "scam" },
      { id: "quang-cao", label: "Quảng cáo sản phẩm", voiceName: "Thùy Dung", voiceSpeed: 1.0, theme: "ad" },
    ];
  }
}

export function discoverSkills(skillRoot, extraRoots = []) {
  const rootExists = Boolean(skillRoot && fs.existsSync(skillRoot));
  const skills = SKILL_PRESETS.map((preset) => {
    const dir = skillRoot ? path.join(skillRoot, preset.id) : "";
    const skillMdPath = dir ? path.join(dir, "SKILL.md") : "";
    const existsOnDisk = Boolean(skillMdPath && fs.existsSync(skillMdPath));
    const fm = existsOnDisk ? parseFrontmatter(fs.readFileSync(skillMdPath, "utf8")) : {};
    return {
      ...preset,
      isCustom: false,
      existsOnDisk,
      skillDir: dir,
      skillMdPath,
      rawDescription: fm.description || preset.summary,
    };
  });

  const scanRoots = [skillRoot, ...(Array.isArray(extraRoots) ? extraRoots : [])]
    .map((r) => String(r || "").trim())
    .filter((r) => r && fs.existsSync(r));

  const seenDirs = new Set();
  for (const rootDir of scanRoots) {
    const normRoot = process.platform === "win32" ? path.resolve(rootDir).toLowerCase() : path.resolve(rootDir);
    if (seenDirs.has(normRoot)) continue;
    seenDirs.add(normRoot);

    try {
      for (const entry of fs.readdirSync(rootDir, { withFileTypes: true })) {
        if (!entry.isDirectory() || entry.name.startsWith("_") || entry.name.startsWith(".")) continue;
        if (skills.some((s) => s.id === entry.name)) continue;
        const sDir = path.join(rootDir, entry.name);
        const md = path.join(sDir, "SKILL.md");
        if (!fs.existsSync(md)) continue;
        const fm = parseFrontmatter(fs.readFileSync(md, "utf8"));
        skills.push({
          id: entry.name,
          title: fm.name || entry.name,
          titleEn: fm.name || entry.name,
          badge: "Skill mở rộng",
          badgeEn: "Imported Skill",
          engine: "Custom / Imported Skill",
          color: "#6366f1",
          defaultFormat: "9:16",
          formats: ["9:16", "16:9", "1:1"],
          defaultTopic: "thoi-su",
          supportsCharacter: true,
          supportsStickMode: false,
          estimatedTime: "~5 phút",
          recommendedProvider: "claude",
          summary: fm.description || `Skill mở rộng lưu tại ${sDir}`,
          summaryEn: fm.description || `Imported skill stored at ${sDir}`,
          rawDescription: fm.description || "",
          isCustom: true,
          existsOnDisk: true,
          skillDir: sDir,
          skillMdPath: md,
          quickTemplates: [
            {
              label: "Chạy theo yêu cầu",
              topic: "thoi-su",
              brief: `Thực hiện tác vụ dựng video / nội dung với skill ${entry.name}: `,
            },
          ],
        });
      }
    } catch {
      // ignore
    }
  }

  return {
    skillRoot,
    extraRoots: scanRoots.filter((r) => path.resolve(r) !== path.resolve(skillRoot || "")),
    rootExists,
    hasShared: Boolean(skillRoot && fs.existsSync(path.join(skillRoot, "_shared", "sync-shared.mjs"))),
    hasVieNeu: Boolean(
      skillRoot &&
        (fs.existsSync(path.join(skillRoot, "_tools", "VieNeu-TTS", ".venv", "Scripts", "python.exe")) ||
          fs.existsSync(path.join(skillRoot, "_tools", "VieNeu-TTS", ".venv", "bin", "python")))
    ),
    topics: loadTopicProfiles(skillRoot || ""),
    skills,
  };
}

/** Ghép câu lệnh chuẩn xác theo quy ước từng AI (/skill cho Claude & Antigravity, $skill cho Codex) */
export function composeSkillPrompt({
  skillId,
  providerId,
  brief,
  topic,
  format,
  character,
  stickMode,
  renderStage = "draft",
  projectFolder = "",
  outputDir = "",
  mediaFiles = [],
  sharedMemoryHint = "",
}) {
  const prefix = providerId === "codex" ? `$${skillId}` : `/${skillId}`;
  const parts = [prefix];

  if (projectFolder && projectFolder.trim()) {
    const cleanFolder = projectFolder.trim();
    if (path.isAbsolute(cleanFolder)) {
      parts.push(`Thư mục lưu dự án: "${cleanFolder}".`);
    } else if (outputDir && outputDir.trim()) {
      const joined = path.join(outputDir.trim(), cleanFolder.replace(/^_video[\\/]/i, ""));
      parts.push(`Thư mục lưu dự án: "${joined}".`);
    } else {
      parts.push(`Thư mục dự án: ${cleanFolder}.`);
    }
  } else if (outputDir && outputDir.trim()) {
    parts.push(`Lưu video đầu ra trong thư mục gốc: "${outputDir.trim()}".`);
  }

  if (stickMode && skillId === "stickman-video") {
    parts.push(`Chế độ --mode ${stickMode}.`);
  }
  if (brief && brief.trim()) {
    parts.push(brief.trim());
  }
  if (Array.isArray(mediaFiles) && mediaFiles.length > 0) {
    const formattedList = mediaFiles
      .map((m) => (typeof m === "string" ? `"${m}"` : `"${m.path}"`))
      .join(", ");
    parts.push(`Sử dụng tư liệu ảnh/video người dùng đã chọn từ thiết bị tại: ${formattedList}.`);
  }

  const specs = [];
  if (format) specs.push(`khổ khung hình ${format}`);
  if (topic && topic !== "auto") specs.push(`meta.topic "${topic}"`);
  if (character) specs.push(`kiểu nhân vật "${character}"`);
  if (specs.length) {
    parts.push(`Cấu hình: ${specs.join(", ")}.`);
  }

  if (renderStage === "draft") {
    parts.push(
      "Chạy đúng quy trình trong SKILL.md & AGENTS.md: tạo giọng VieNeu-TTS (tts.mjs --print), dựng timeline & trộn âm -14 LUFS, render nháp (draft), chạy measure.mjs + qc-sync.mjs và kiểm tra tờ ảnh .sheet.jpg rồi dừng báo cáo để tôi duyệt."
    );
  } else {
    parts.push(
      "Chạy trọn gói theo SKILL.md & AGENTS.md: đối chiếu nguồn trong FACTS.md, che mặt bị can nếu là vụ án (blur-faces.mjs), tạo giọng VieNeu-TTS, render nháp kiểm tra .sheet.jpg + qc-sync.mjs, sau đó xuất bản cuối 1080p (final), chạy measure.mjs nghiệm thu toàn bộ ✓ và cập nhật HANDOFF.md."
    );
  }

  if (sharedMemoryHint && sharedMemoryHint.trim()) {
    parts.push(sharedMemoryHint.trim());
  }

  return parts.join(" ");
}
