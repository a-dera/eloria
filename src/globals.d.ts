/** Application version, injected by Vite from package.json. */
declare const __APP_VERSION__: string;

/** Raw source of a module (Vite `?raw`). */
declare module "*?raw" {
  const src: string;
  export default src;
}
