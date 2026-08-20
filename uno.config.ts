import { defineConfig, presetWind3 } from "unocss";

export default defineConfig({
  presets: [presetWind3({ preflight: false })],
  preflights: [],
  variants: [
    (matcher) => {
      if (!matcher.startsWith("mobile:")) {
        return;
      }

      return {
        matcher: matcher.slice("mobile:".length),
        parent: "@media (max-width: 767px)",
      };
    },
  ],
  content: {
    pipeline: false,
  },
  extractorDefault: null,
  safelist: [
    "min-w-0",
    "block",
    "my-2",
    "truncate",
    "m-0",
    "mb-2",
    "grid",
    "gap-3",
    "mobile:gap-2.5",
    "w-full",
    "mobile:flex-col-reverse",
  ],
});
