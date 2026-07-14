import { defineConfig } from "@rslib/core";

export default defineConfig({
  lib: [
    {
      format: "esm",
      syntax: "es2022",
      dts: true,
    },
    {
      format: "cjs",
      syntax: "es2022",
    },
  ],
  source: {
    tsconfigPath: "./tsconfig.build.json",
  },
  output: {
    externals: ["node:fs/promises"],
    target: "web",
    sourceMap: true,
  },
});
