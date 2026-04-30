import { BrowserRouter, Routes, Route, useLocation, useNavigate } from "react-router-dom";
import { MainLayout } from "@/components/layout/MainLayout";
import { DashboardPage } from "@/pages/DashboardPage";
import { ProjectsPage } from "@/pages/ProjectsPage";
import { ProjectDetailPage } from "@/pages/ProjectDetailPage";
import { KanbanPage } from "@/pages/KanbanPage";
import { EmployeesPage } from "@/pages/EmployeesPage";
import { SessionsPage } from "@/pages/SessionsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { TrashPage } from "@/pages/TrashPage";
import { ShortcutsHelpDialog } from "@/components/keyboard/ShortcutsHelpDialog";
import { NAV_SHORTCUTS, shortcutKeys } from "@/lib/shortcuts";
import "@/index.css";
import { useEffect, useRef } from "react";
import { useHotkeys } from "react-hotkeys-hook";

const LAST_ROUTE_STORAGE_KEY = "codex-ai:last-route";

function readStoredRoute() {
  if (typeof window === "undefined") {
    return null;
  }

  const route = window.localStorage.getItem(LAST_ROUTE_STORAGE_KEY);
  if (!route || !route.startsWith("/")) {
    return null;
  }

  return route;
}

function persistCurrentRoute(route: string) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(LAST_ROUTE_STORAGE_KEY, route);
}

function GlobalShortcuts() {
  const navigate = useNavigate();

  useHotkeys(shortcutKeys(NAV_SHORTCUTS[0]), () => navigate("/"), { preventDefault: true });
  useHotkeys(shortcutKeys(NAV_SHORTCUTS[1]), () => navigate("/projects"), { preventDefault: true });
  useHotkeys(shortcutKeys(NAV_SHORTCUTS[2]), () => navigate("/kanban"), { preventDefault: true });
  useHotkeys(shortcutKeys(NAV_SHORTCUTS[3]), () => navigate("/sessions"), { preventDefault: true });
  useHotkeys(shortcutKeys(NAV_SHORTCUTS[4]), () => navigate("/employees"), { preventDefault: true });
  useHotkeys(shortcutKeys(NAV_SHORTCUTS[5]), () => navigate("/settings"), { preventDefault: true });
  useHotkeys(shortcutKeys(NAV_SHORTCUTS[6]), () => navigate("/trash"), { preventDefault: true });

  return null;
}

function RoutePersistence() {
  const location = useLocation();
  const navigate = useNavigate();
  const restoredRef = useRef(false);
  const skipPersistRef = useRef(false);

  useEffect(() => {
    if (restoredRef.current) {
      return;
    }

    restoredRef.current = true;
    const currentRoute = `${location.pathname}${location.search}`;

    if (location.pathname !== "/" || location.search) {
      return;
    }

    const storedRoute = readStoredRoute();
    if (storedRoute && storedRoute !== currentRoute) {
      skipPersistRef.current = true;
      navigate(storedRoute, { replace: true });
    }
  }, [location.pathname, location.search, navigate]);

  useEffect(() => {
    if (skipPersistRef.current) {
      skipPersistRef.current = false;
      return;
    }

    persistCurrentRoute(`${location.pathname}${location.search}`);
  }, [location.pathname, location.search]);

  return null;
}

function App() {
  return (
    <BrowserRouter>
      <GlobalShortcuts />
      <RoutePersistence />
      <Routes>
        <Route element={<MainLayout />}>
          <Route path="/" element={<DashboardPage />} />
          <Route path="/projects" element={<ProjectsPage />} />
          <Route path="/projects/:id" element={<ProjectDetailPage />} />
          <Route path="/kanban" element={<KanbanPage />} />
          <Route path="/sessions" element={<SessionsPage />} />
          <Route path="/employees" element={<EmployeesPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/trash" element={<TrashPage />} />
        </Route>
      </Routes>
      <ShortcutsHelpDialog />
    </BrowserRouter>
  );
}

export default App;
