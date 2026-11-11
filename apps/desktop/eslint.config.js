import js from "@eslint/js";
import pluginVue from "eslint-plugin-vue";
import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript";
import prettier from "eslint-config-prettier";
import globals from "globals";

export default defineConfigWithVueTs(
  {
    // src/generated is produced by scripts/gen-types.sh (typeshare): never edited, never linted.
    ignores: ["dist/**", "src-tauri/**", "src/generated/**"],
  },
  js.configs.recommended,
  pluginVue.configs["flat/recommended"],
  vueTsConfigs.recommended,
  {
    languageOptions: { globals: globals.browser },
  },
  {
    // End-to-end tests run in Node (scripts/desktop-e2e.sh).
    files: ["e2e/**/*.ts"],
    languageOptions: { globals: globals.node },
  },
  prettier,
);
