import { createApp } from "vue";
// Fonts are bundled with the app (no CDN): Latin subset only.
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-500.css";
import "@fontsource/inter/latin-600.css";
import "@fontsource/space-grotesk/latin-500.css";
import "@fontsource/space-grotesk/latin-600.css";
import "./style.css";
import App from "./App.vue";
import { i18n } from "./i18n";

createApp(App).use(i18n).mount("#app");
