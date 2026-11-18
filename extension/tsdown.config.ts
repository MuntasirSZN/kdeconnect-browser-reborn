import { defineConfig } from "tsdown";

export default defineConfig({
  entry: {
    background: "src/background.ts",
    "popup/main": "src/popup/main.ts",
    "options/main": "src/options/main.ts",
  },
  outDir: "dist",
  format: "esm",
  target: "es2022",
  platform: "browser",
  clean: true,
  sourcemap: true,
  dts: false,
  copy: [
    "static/manifest.json",
    "static/popup.html",
    "static/options.html",
    { from: "src/popup/style.css", to: "dist/popup" },
    { from: "src/options/style.css", to: "dist/options" },
    { from: "static/images", to: "dist/", flatten: false },
  ],
});
