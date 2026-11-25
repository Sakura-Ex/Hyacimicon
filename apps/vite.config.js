import { defineConfig } from "vite";
import { readFileSync } from "node:fs";

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
  build: { emptyOutDir: true },
});
