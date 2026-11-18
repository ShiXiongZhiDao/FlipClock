import { onBeforeUnmount, onMounted, ref } from "vue";

/**
 * 只负责“现在几点”。对齐到下一秒边界更新，不用会累积漂移的 setInterval(1000)，
 * 因此每一秒的翻页都踩在真实的秒边界上。
 */
export function useClock() {
  const now = ref(new Date());
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;

  const schedule = () => {
    if (disposed) return;
    const rest = 1000 - (Date.now() % 1000);
    timer = setTimeout(() => {
      now.value = new Date();
      schedule();
    }, rest + 4);
  };

  const resync = () => {
    now.value = new Date();
  };

  onMounted(() => {
    now.value = new Date();
    schedule();
    document.addEventListener("visibilitychange", resync);
  });

  onBeforeUnmount(() => {
    disposed = true;
    if (timer) clearTimeout(timer);
    document.removeEventListener("visibilitychange", resync);
  });

  return { now };
}
