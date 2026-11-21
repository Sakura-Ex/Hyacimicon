import { defineConfig } from "vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export default defineConfig({
  plugins: [
    {
      name: "bundle-font-licenses",
      generateBundle() {
        for (const name of ["noto-sans-sc", "noto-serif-sc"]) {
          const license = new URL(
            "./node_modules/@fontsource-variable/" + name + "/LICENSE",
            import.meta.url,
          );
          this.emitFile({
            type: "asset",
            fileName: "licenses/" + name + ".txt",
            source: readFileSync(license),
          });
        }
      },
    },
  ],
  publicDir: fileURLToPath(
    new URL("../../design-drafts/player-app/assets/", import.meta.url),
  ),
});
