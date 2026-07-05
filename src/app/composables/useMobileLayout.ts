import { onBeforeUnmount, onMounted, ref } from "vue";

const MOBILE_LAYOUT_QUERY = "(max-width: 767px)";

export function useMobileLayout() {
  const isMobileLayout = ref(false);
  let mediaQuery: MediaQueryList | null = null;

  function updateMatches(event?: MediaQueryListEvent) {
    if (event) {
      isMobileLayout.value = event.matches;
      return;
    }

    isMobileLayout.value = mediaQuery?.matches ?? false;
  }

  onMounted(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }

    mediaQuery = window.matchMedia(MOBILE_LAYOUT_QUERY);
    updateMatches();
    mediaQuery.addEventListener("change", updateMatches);
  });

  onBeforeUnmount(() => {
    mediaQuery?.removeEventListener("change", updateMatches);
    mediaQuery = null;
  });

  return {
    isMobileLayout,
  };
}
