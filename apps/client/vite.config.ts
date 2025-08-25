import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { resolve } from "node:path";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    // Exclude worker and non-React markdown utils from React Fast Refresh
    react({
      fastRefresh: true,
      exclude: [/src\/workers\//, /src\/lib\/markdown\//]
    }),
    tailwindcss(),
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true
    })
  ],

  build: {
    sourcemap: !!process.env.TAURI_ENV_DEBUG
  },

  worker: {
    format: "es" as const,
    plugins: () => [
      // Exclude React plugin from workers to prevent HMR issues
      tailwindcss()
    ]
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp"
    },
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"]
    }
  },
  resolve: {
    alias: {
      "@": resolve(__dirname, "./src")
    }
  }
}));
