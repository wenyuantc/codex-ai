import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { applyTheme, getThemePreference } from "@/lib/theme";

applyTheme(getThemePreference());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
