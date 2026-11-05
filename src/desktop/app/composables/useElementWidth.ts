import type { Ref } from 'vue';

/** 跟踪元素的 CSS 宽度（px），让 SVG 图表按 1:1 绘制，文字不随容器缩放。 */
export function useElementWidth(el: Ref<HTMLElement | null>, fallback = 600) {
  const width = ref(fallback);
  let ro: ResizeObserver | null = null;
  onMounted(() => {
    if (!el.value) return;
    width.value = Math.max(200, Math.round(el.value.clientWidth)) || fallback;
    ro = new ResizeObserver((entries) => {
      const w = Math.round(entries[0]?.contentRect.width ?? 0);
      if (w > 0) width.value = Math.max(200, w);
    });
    ro.observe(el.value);
  });
  onBeforeUnmount(() => ro?.disconnect());
  return width;
}
