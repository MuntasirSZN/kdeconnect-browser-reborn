import { defineConfig } from "vitest/config";
import { WxtVitest } from "wxt/testing/vitest-plugin";
import { createOxcInstrumenter } from "oxc-coverage-instrument/vitest";

export default defineConfig({
  plugins: [WxtVitest()],
  test: {
    coverage: {
      provider: "istanbul",
      instrumenter: (options) => createOxcInstrumenter({ ...options, compat: "istanbul" }),
    },
    include: ["tests/**/*.test.ts"],
  },
});
