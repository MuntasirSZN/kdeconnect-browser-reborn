import { defineBackground } from "#imports";

export default defineBackground({
  type: "module",
  main() {
    void import("../background/index");
  },
});
