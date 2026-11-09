import "@fontsource-variable/inter";
import "@fontsource-variable/literata";
import "@fontsource-variable/literata/wght-italic.css";
import "@fontsource-variable/source-serif-4";
import "@fontsource-variable/lora";
import "@fontsource-variable/jetbrains-mono";
import "katex/dist/katex.min.css";
import "./app.css";
import { mount } from "svelte";
import { installDevlog } from "./lib/devlog.svelte";
import App from "./App.svelte";

// Developer console log, before everything else.
installDevlog();

const target = document.getElementById("app")!;
export default mount(App, { target });
