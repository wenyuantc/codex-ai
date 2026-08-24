import React from "react";
import ReactDOM from "react-dom/client";
import { I18nextProvider } from "react-i18next";

import App from "./App";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import i18n from "@/lib/i18n";
import { applyTheme, getThemePreference } from "@/lib/theme";

applyTheme(getThemePreference());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nextProvider i18n={i18n}>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </I18nextProvider>
  </React.StrictMode>,
);
