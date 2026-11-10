<script setup lang="ts">
// Draws a stored icon (SVG path data on a 24 × 24 grid, Lucide stroke style),
// exactly like the tablet does.
defineProps<{ paths: string[]; size: number; color: string }>();
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    :stroke="color"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-for="(d, i) in paths" :key="i" :d="d" />
  </svg>
</template>
