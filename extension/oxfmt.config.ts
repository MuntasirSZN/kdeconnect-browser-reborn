import { defineConfig } from "oxfmt";

export default defineConfig({
  ignorePatterns: ["src/generated/**/*", "src/locales/**/*", ".output/**/*", ".wxt/**/*"],
  sortImports: true,
});
