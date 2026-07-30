import "./style.css";
import "./events";
import { route } from "./router";
import { loadSession } from "./session";

window.addEventListener("popstate", () => void route());
void loadSession().then(route);
