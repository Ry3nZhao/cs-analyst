import { createLogger } from 'vite';

// Harmless upstream warnings during production build (@nuxt/ui → @vueuse/core, Nuxt preload polyfill).
const viteLogger = createLogger();
const viteWarn = viteLogger.warn.bind(viteLogger);
viteLogger.warn = (msg, options) => {
  if (msg.includes('module-preload-polyfill') || msg.includes('#__PURE__')) {
    return;
  }
  viteWarn(msg, options);
};

export default defineNuxtConfig({
  compatibilityDate: '2025-05-15',
  modules: ['@nuxt/ui'],
  ssr: false,
  telemetry: false,
  sourcemap: {
    client: false,
  },
  devServer: { host: '0', port: 3000 },
  vite: {
    customLogger: viteLogger,
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
    server: { strictPort: true },
  },
  ignore: ['**/src-tauri/**'],
  css: ['~/assets/css/main.css'],
  app: {
    head: {
      title: 'CS Demo Analyst',
    },
  },
});
