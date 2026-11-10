<script setup lang="ts">
import { ref, watch } from "vue";

// A text field whose edits round-trip through the desktop. While focused it
// keeps its own draft: a late echo of an earlier keystroke must not overwrite
// what the user typed since. It follows the stored value again once left.
const props = defineProps<{ value: string; maxlength?: number; placeholder?: string }>();
const emit = defineEmits<{ input: [value: string]; commit: [] }>();

const draft = ref(props.value);
const focused = ref(false);
watch(
  () => props.value,
  (v) => {
    if (!focused.value) draft.value = v;
  },
);

function onInput(event: Event) {
  draft.value = (event.target as HTMLInputElement).value;
  emit("input", draft.value);
}

function onBlur() {
  focused.value = false;
  draft.value = props.value;
  emit("commit");
}
</script>

<template>
  <input
    :value="draft"
    :maxlength="maxlength"
    :placeholder="placeholder"
    @focus="focused = true"
    @input="onInput"
    @blur="onBlur"
  />
</template>
