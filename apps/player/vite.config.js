import { defineConfig } from "vite";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const draftAssets = new URL("../../design-drafts/player-app/assets/", import.meta.url);

export default defineConfig({
  plugins: [
    {
      name: "bundle-book-and-controls",
      generateBundle() {
        for (const folder of ["book", "ui"]) {
          const directory = new URL(folder + "/", draftAssets);
          for (const name of readdirSync(directory)) {
            this.emitFile({
              type: "asset",
              fileName: folder + "/" + name,
              source: readFileSync(new URL(name, directory)),
            });
          }
        }
      },
    },
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
  publicDir: fileURLToPath(draftAssets),
  build: { copyPublicDir: false, emptyOutDir: true },
});
