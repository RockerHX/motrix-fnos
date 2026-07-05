import { createPinia } from "pinia";
import { createApp } from "vue";
import App from "./App.vue";
import "./styles/mobile-baseline.css";

createApp(App).use(createPinia()).mount("#app");
