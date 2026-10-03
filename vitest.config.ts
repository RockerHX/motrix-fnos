import { configDefaults, defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      exclude: [...configDefaults.exclude, "scripts/tests/**", "scripts/extended/**", "e2e/**"],
      setupFiles: ["src/test/setup.ts"],
      clearMocks: true,
      restoreMocks: true,
      unstubGlobals: true,
    },
  }),
);
