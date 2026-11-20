/**
 * 页面级短提示。
 *
 * 一处只留一条，重复触发会重置消失计时。设置页、外观页与主题编辑器
 * 各自持有一份实例——它们分属不同窗口/页面，不需要共享状态。
 */
import { onBeforeUnmount, ref } from "vue";

export function useToast(durationMs = 2600) {
  const toast = ref<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function notify(text: string): void {
    toast.value = text;
    clearTimeout(timer);
    timer = setTimeout(() => {
      toast.value = null;
    }, durationMs);
  }

  onBeforeUnmount(() => clearTimeout(timer));

  return { toast, notify };
}
