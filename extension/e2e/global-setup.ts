import { execFileSync } from "node:child_process";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

/** Build the extension with the pinned E2E key before the suite starts. */
export default function globalSetup(): void {
  execFileSync("bun", ["run", "build:chrome"], {
    cwd: HERE,
    stdio: "inherit",
    env: { ...process.env, E2E: "1" },
  });
}
