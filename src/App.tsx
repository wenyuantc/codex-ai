import { BrowserRouter, Routes, Route, useLocation, useNavigate } from "react-router-dom";
import { MainLayout } from "@/components/layout/MainLayout";
import { DashboardPage } from "@/pages/DashboardPage";
import { ProjectsPage } from "@/pages/ProjectsPage";
import { ProjectDetailPage } from "@/pages/ProjectDetailPage";
import { KanbanPage } from "@/pages/KanbanPage";
import { EmployeesPage } from "@/pages/EmployeesPage";
import { SessionsPage } from "@/pages/SessionsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import "@/index.css";
import { useEffect, useRef } from "react";

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

function KeyboardShortcuts() {
  const navigate = useNavigate();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Only trigger if no input/textarea is focused
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;

      const isMod = e.metaKey || e.ctrlKey;

      if (isMod && e.key === "n") {
        e.preventDefault();
        navigate("/kanban");
      } else if (isMod && e.key === "e") {
        e.preventDefault();
        navigate("/employees");
      } else if (isMod && e.key === "d") {
        e.preventDefault();
        navigate("/");
      } else if (isMod && e.key === "p") {
        e.preventDefault();
        navigate("/projects");
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [navigate]);

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
      <KeyboardShortcuts />
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
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export default App;
