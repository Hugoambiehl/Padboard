import { onMounted, onUnmounted, ref } from "vue";

/** Current time in ms, refreshed every `intervalMs` while the component is mounted. */
export function useNow(intervalMs = 1000) {
  const now = ref(Date.now());
  let timer: ReturnType<typeof setInterval> | undefined;
  onMounted(() => {
    timer = setInterval(() => (now.value = Date.now()), intervalMs);
  });
  onUnmounted(() => clearInterval(timer));
  return now;
}
