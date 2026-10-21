import { defineConfig } from "vite";

// Servidor local solo para desarrollo con Tauri; nada se sirve fuera del equipo.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true, host: "127.0.0.1" },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
