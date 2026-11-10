import { createI18n } from "vue-i18n";
import fr from "./locales/fr.json";

/** Shape of the messages: every locale must provide the same keys as `fr`. */
export type MessageSchema = typeof fr;

export const i18n = createI18n<[MessageSchema], "fr">({
  legacy: false,
  locale: "fr",
  fallbackLocale: "fr",
  messages: { fr },
  datetimeFormats: {
    fr: {
      time: { hour: "2-digit", minute: "2-digit", second: "2-digit" },
      dateTime: { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" },
    },
  },
});
