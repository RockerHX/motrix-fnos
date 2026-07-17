<script setup lang="ts">
import { computed, useSlots } from "vue";

const props = withDefaults(
  defineProps<{
    title?: string;
    description?: string;
  }>(),
  {
    title: "",
    description: "",
  },
);

const slots = useSlots();
const hasHeader = computed(() => Boolean(props.title || slots.title || slots.meta || slots.actions));
const hasDescription = computed(() => Boolean(props.description || slots.description));
</script>

<template>
  <section class="app-section-card">
    <div v-if="hasHeader" class="app-section-card__header">
      <div class="app-section-card__title-block">
        <h3 v-if="props.title || $slots.title" class="app-section-card__title">
          <slot name="title">{{ props.title }}</slot>
        </h3>
        <p v-if="hasDescription" class="app-section-card__description">
          <slot name="description">{{ props.description }}</slot>
        </p>
      </div>
      <div v-if="$slots.meta || $slots.actions" class="app-section-card__aside">
        <slot name="meta" />
        <slot name="actions" />
      </div>
    </div>

    <slot />
  </section>
</template>

<style scoped src="./AppSectionCard.css"></style>
