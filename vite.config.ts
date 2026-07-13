import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

const serverTarget = "http://127.0.0.1:17080";
const webBase = process.env.MOTRIX_FNOS_WEB_BASE || "./";

// https://vite.dev/config/
export default defineConfig({
  base: webBase,
  plugins: [vue()],
  clearScreen: false,
  build: {
    chunkSizeWarningLimit: 800,
    rollupOptions: {
      output: {
        manualChunks: {
          naive: ["naive-ui"],
          vendor: ["vue", "pinia"],
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    proxy: {
      "/api": {
        target: serverTarget,
        changeOrigin: false,
      },
      "/events": {
        target: serverTarget,
        changeOrigin: false,
        rewrite: () => "/api/events",
      },
    },
  },
});
