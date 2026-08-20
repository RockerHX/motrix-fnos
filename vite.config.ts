import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import UnoCSS from "unocss/vite";

const serverTarget = "http://127.0.0.1:17080";

// https://vite.dev/config/
export default defineConfig({
  base: "./",
  plugins: [UnoCSS(), vue()],
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
