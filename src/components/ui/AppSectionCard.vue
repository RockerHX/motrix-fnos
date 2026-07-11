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

<style scoped>
.app-section-card {
  min-width: 0;
  display: grid;
  gap: 14px;
  padding: 16px;
  border-radius: var(--app-radius-md);
  background: var(--app-color-card-overlay);
}

.app-section-card__header {
  min-width: 0;
  display: flex;
  justify-content: space-between;
  gap: 16px;
}

.app-section-card__title-block {
  min-width: 0;
  display: grid;
  gap: 6px;
}

.app-section-card__title,
.app-section-card__description {
  margin: 0;
  overflow-wrap: anywhere;
}

.app-section-card__title {
  color: var(--app-text-strong);
  font-size: 18px;
  font-weight: 700;
}

.app-section-card__description {
  color: var(--app-text-muted);
  font-size: 14px;
  line-height: 1.7;
}

.app-section-card__aside {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 10px;
}

@media (max-width: 767px) {
  .app-section-card {
    gap: 12px;
    padding: 14px;
  }

  .app-section-card__header {
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 12px;
  }

  .app-section-card__title {
    font-size: 15px;
  }
}
</style>
