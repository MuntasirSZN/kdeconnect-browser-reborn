import { expect, test } from "bun:test";
import { typeIcon } from "./icons";

test("maps KDE Connect types to Lucide names", () => {
  expect(typeIcon("phone")).toBe("smartphone");
  expect(typeIcon("smartphone")).toBe("smartphone");
  expect(typeIcon("tablet")).toBe("tablet");
  expect(typeIcon("laptop")).toBe("laptop");
  expect(typeIcon("desktop")).toBe("monitor");
  expect(typeIcon("tv")).toBe("tv");
  expect(typeIcon("something-else")).toBe("monitor-smartphone");
});
