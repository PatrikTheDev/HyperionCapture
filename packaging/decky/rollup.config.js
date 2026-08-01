import deckyPlugin from "@decky/rollup";
import esbuild from "rollup-plugin-esbuild";

const config = deckyPlugin();

// @decky/rollup 1.0.2's TypeScript plugin uses a watch program even for a
// one-shot build, which can leave a timer alive and hang CI. `bun run build`
// type-checks with `tsc --noEmit`; this plugin only performs the transpilation.
config.plugins = config.plugins.map((plugin) =>
  plugin.name === "typescript"
    ? esbuild({ target: "es2020", jsx: "automatic" })
    : plugin,
);

export default config;
