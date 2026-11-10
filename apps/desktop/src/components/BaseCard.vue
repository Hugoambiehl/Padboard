<script setup lang="ts">
import type { Component } from "vue";

defineProps<{ title: string; icon: Component }>();
</script>

<template>
  <section class="flex flex-col gap-4 rounded-2xl border border-line bg-surface p-5">
    <h2 class="flex items-center gap-2.5 font-display text-base font-semibold">
      <component :is="icon" :size="18" class="text-muted-2" aria-hidden="true" />
      {{ title }}
    </h2>
    <slot />
  </section>
</template>
