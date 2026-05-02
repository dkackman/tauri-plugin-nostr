import tseslint from "typescript-eslint";
import svelte from "eslint-plugin-svelte";

export default tseslint.config(
  { ignores: ["dist-js/", "**/dist/", "**/node_modules/", "**/target/"] },
  ...tseslint.configs.recommended,
  { files: ["guest-js/**/*.ts"] },
  ...svelte.configs["flat/recommended"],
  { files: ["examples/tauri-app/src/**/*.svelte"] }
);
