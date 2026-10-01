import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const target = "src/generated/api.d.ts";
const npx = process.platform === "win32" ? "npx.cmd" : "npx";

function run(args, input) {
  const result = spawnSync(npx, ["--no-install", ...args], {
    cwd: root,
    encoding: "utf8",
    input,
  });
  if (result.status !== 0) {
    process.stderr.write(result.stderr || result.stdout);
    process.exit(result.status ?? 1);
  }
  return result.stdout;
}

const generated = run(["openapi-typescript", "../openapi/meerkateer.openapi.json"]);
const formatted = run(["biome", "format", "--stdin-file-path", target], generated);
const current = readFileSync(new URL(`../${target}`, import.meta.url), "utf8");

if (formatted !== current) {
  process.stderr.write(
    `Generated API types are stale. Run "npm run generate:api" and commit ${target}.\n`,
  );
  process.exit(1);
}

process.stdout.write("Generated API types match the OpenAPI document.\n");
