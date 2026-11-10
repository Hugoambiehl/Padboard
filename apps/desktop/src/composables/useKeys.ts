import { ref } from "vue";
import { listKeys } from "../commands";

// The key names the desktop can inject, fetched once.
const keys = ref<string[]>([]);
let loading: Promise<void> | null = null;

export function useKeys() {
  loading ??= listKeys()
    .then((list) => {
      keys.value = list;
    })
    .catch((e) => console.error("cannot list keys", e));
  return keys;
}
