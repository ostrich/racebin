import "./style.css";
import "./events";
import { hasUnsavedChanges } from "./navigation_guard";
import { handlePopState, route } from "./router";
import { loadSession } from "./session";

window.addEventListener("popstate", () => handlePopState(document.body.dataset.routePath ?? "/"));
window.addEventListener("beforeunload", event => {
  if (!hasUnsavedChanges()) return;
  event.preventDefault();
  event.returnValue = "";
});
void loadSession().then(route);
