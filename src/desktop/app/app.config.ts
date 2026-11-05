// @nuxt/ui 主题：主色用中性灰，彩色只留给语义（success / warning / error / info）。
// 具体色值由 assets/css/main.css 里的 --ui-* 覆盖，与应用 token 保持一致。
export default defineAppConfig({
  ui: {
    colors: {
      primary: 'neutral',
      neutral: 'neutral',
      success: 'green',
      warning: 'amber',
      error: 'red',
      info: 'blue',
    },
  },
});
