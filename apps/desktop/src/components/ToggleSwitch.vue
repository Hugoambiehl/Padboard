<script setup lang="ts">
defineProps<{ checked: boolean; disabled?: boolean; label: string }>();
defineEmits<{ toggle: [value: boolean] }>();
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="checked"
    :aria-label="label"
    :disabled="disabled"
    class="relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors disabled:cursor-not-allowed disabled:opacity-50"
    :class="checked ? 'border-accent bg-accent' : 'border-line-2 bg-surface-active'"
    @click="$emit('toggle', !checked)"
  >
    <span
      class="size-4.5 rounded-full bg-fg shadow transition-transform"
      :class="checked ? 'translate-x-5.5' : 'translate-x-0.5'"
    />
  </button>
</template>
