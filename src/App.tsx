import { BrowserRouter, Routes, Route, useNavigate } from "react-router-dom";
import { MainLayout } from "@/components/layout/MainLayout";
import { DashboardPage } from "@/pages/DashboardPage";
import { ProjectsPage } from "@/pages/ProjectsPage";
import { ProjectDetailPage } from "@/pages/ProjectDetailPage";
import { KanbanPage } from "@/pages/KanbanPage";
import { EmployeesPage } from "@/pages/EmployeesPage";
import { SessionsPage } from "@/pages/SessionsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import "@/index.css";
import { useEffect } from "react";

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

function App() {
  return (
    <BrowserRouter>
      <KeyboardShortcuts />
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
