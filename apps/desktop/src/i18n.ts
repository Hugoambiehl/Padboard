import type { Ref } from "vue";
import { createI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import fr from "./locales/fr.json";
import en from "./locales/en.json";

/** Shape of the messages: every locale provides the same keys (locales.test.ts). */
export type MessageSchema = typeof fr;
export type AppLocale = "fr" | "en";

const datetime = {
  time: { hour: "2-digit", minute: "2-digit", second: "2-digit" },
  dateTime: { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" },
} as const;

export const i18n = createI18n<[MessageSchema], AppLocale>({
  legacy: false,
  locale: "fr",
  fallbackLocale: "en",
  messages: { fr, en },
  datetimeFormats: { fr: datetime, en: datetime },
});

/**
 * The UI follows the PC's language, as decided by the backend (`get_locale`),
 * so the editor and the texts sent to the tablets always agree.
 */
export async function useBackendLocale(): Promise<void> {
  let locale: AppLocale = "en";
  try {
    locale = (await invoke<AppLocale>("get_locale")) === "fr" ? "fr" : "en";
  } catch (e) {
    console.warn("locale unavailable, English used", e);
  }
  // Composition mode (legacy: false): the global locale is a ref.
  (i18n.global.locale as unknown as Ref<AppLocale>).value = locale;
  document.documentElement.lang = locale;
}
