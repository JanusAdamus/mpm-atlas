import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { defineConfig, type Plugin } from "vite";

// The server sends `.br` / `.gz` siblings when they exist (tower-http precompressed),
// so text assets are compressed once at build time at maximum quality.
function precompress(): Plugin {
  return {
    name: "atlas-precompress",
    apply: "build",
    // "post": Vite rewrites the dynamic-import preload placeholder (__VITE_PRELOAD__) in its own
    // generateBundle; compressing before that shipped .br/.gz with the placeholder still in.
    generateBundle: { order: "post", handler(_, bundle) {
      for (const [name, out] of Object.entries(bundle)) {
        if (!/\.(js|css|html|svg|json)$/.test(name)) continue;
        const src = out.type === "chunk" ? out.code : out.source;
        const buf = typeof src === "string" ? Buffer.from(src) : Buffer.from(src);
        if (buf.length < 1024) continue;
        this.emitFile({ type: "asset", fileName: `${name}.br`, source: brotliCompressSync(buf, { params: { [constants.BROTLI_PARAM_QUALITY]: 11 } }) });
        this.emitFile({ type: "asset", fileName: `${name}.gz`, source: gzipSync(buf, { level: 9 }) });
      }
    } },
  };
}

export default defineConfig({
  plugins: [precompress()],
  build: { outDir: "dist", emptyOutDir: true, target: "es2023", assetsInlineLimit: 0, sourcemap: false },
  server: {
    port: 5173,
    fs: { allow: [".."] }, // shapes/vocab.json lives one level up
    proxy: { "/api": "http://127.0.0.1:8765" },
  },
});
