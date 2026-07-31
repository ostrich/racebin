import { mount } from "svelte";
import App from "./App.svelte";
import { initializeUiPreferences } from "./uiPreferences";
import "./style.css";

const target = document.querySelector<HTMLDivElement>("#app");
if (!target) throw new Error("Racebin application root is missing");

initializeUiPreferences();
mount(App, { target });
