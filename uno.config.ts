import { defineConfig, presetWind3 } from "unocss";

export default defineConfig({
  presets: [presetWind3({ preflight: false })],
  preflights: [],
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
    "lt-md:gap-2.5",
    "w-full",
    "lt-md:flex-col-reverse",
  ],
});
