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
