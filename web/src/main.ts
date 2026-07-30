import "./style.css";
import "./events";
import { hasUnsavedChanges } from "./navigation_guard";
import {
  handlePopState,
  initialRoute,
  initializeScrollRestoration
} from "./router";
import { loadSession } from "./session";

initializeScrollRestoration();
window.addEventListener("popstate", event => {
  handlePopState(document.body.dataset.routePath ?? "/", event.state);
});
window.addEventListener("beforeunload", event => {
  if (!hasUnsavedChanges()) return;
  event.preventDefault();
  event.returnValue = "";
});
void loadSession().then(initialRoute);
