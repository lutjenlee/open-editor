import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    exclude: ["**/node_modules/**", "**/.build/**", "**/dist/**"],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/.open-editor/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: "safari16" },
});
