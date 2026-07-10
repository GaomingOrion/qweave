import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Assets are served from the axum static mount at "/", so keep paths relative.
// In dev, proxy the API to the standalone server (qweave-server --port 8080).
export default defineConfig({
  base: "./",
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:8080",
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
