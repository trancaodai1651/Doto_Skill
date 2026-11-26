// Tạo binary Node.js macOS Universal (x86_64 Intel + arm64 Apple Silicon) bằng lipo và ký ad-hoc codesign.
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = path.join(ROOT, ".desktop");
const NODE_BIN = path.join(OUT, "node", "node");
const CACHE = path.join(OUT, "universal-downloads");
fs.mkdirSync(CACHE, { recursive: true });

const version = process.version;
const baseUrl = `https://nodejs.org/dist/${version}`;
console.log(`▸ Tải Node ${version} darwin-x64 & darwin-arm64 để tạo macOS Universal binary...`);

const sumsText = await (await fetch(`${baseUrl}/SHASUMS256.txt`)).text();
for (const arch of ["x64", "arm64"]) {
  const tarName = `node-${version}-darwin-${arch}.tar.gz`;
  const resp = await fetch(`${baseUrl}/${tarName}`);
  if (!resp.ok) throw new Error(`Không tải được ${tarName}: HTTP ${resp.status}`);
  const bytes = Buffer.from(await resp.arrayBuffer());
  const sha = crypto.createHash("sha256").update(bytes).digest("hex");
  if (!sumsText.split("\n").some((l) => l.trim() === `${sha}  ${tarName}`)) {
    throw new Error(`Sai lệch mã băm SHA256 cho ${tarName}`);
  }
  const archive = path.join(CACHE, tarName);
  fs.writeFileSync(archive, bytes);
  execFileSync("tar", ["-xzf", archive, "-C", CACHE]);
}

execFileSync("lipo", [
  "-create",
  path.join(CACHE, `node-${version}-darwin-x64`, "bin", "node"),
  path.join(CACHE, `node-${version}-darwin-arm64`, "bin", "node"),
  "-output",
  NODE_BIN,
]);
fs.chmodSync(NODE_BIN, 0o755);
execFileSync("lipo", [NODE_BIN, "-verify_arch", "x86_64", "arm64"]);
execFileSync("codesign", ["--force", "--sign", "-", NODE_BIN]);
fs.rmSync(CACHE, { recursive: true, force: true });

console.log("✓ Đã tạo thành công Node macOS Universal (x86_64 + arm64)!");
