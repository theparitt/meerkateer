import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

export default defineConfig(({ mode }) => {
  const environment = loadEnv(mode, process.cwd(), "VITE_");
  const apiTarget = environment.VITE_API_PROXY_TARGET || "http://127.0.0.1:6510";
  return {
    cacheDir: "/tmp/meerkateer-vite-cache",
    publicDir: "../art",
    plugins: [react()],
    server: {
      host: "127.0.0.1",
      port: 5173,
      fs: { allow: [".."] },
      proxy: {
        "/health": apiTarget,
        "/ready": apiTarget,
        "/server-info": apiTarget,
        "/openapi.json": apiTarget,
        "/v1": apiTarget,
      },
    },
    test: {
      environment: "jsdom",
    },
  };
});
