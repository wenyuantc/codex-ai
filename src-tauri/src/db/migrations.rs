use tauri_plugin_sql::Migration;

pub fn get_all_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create projects table",
            sql: r#"
                CREATE TABLE projects (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    description TEXT,
                    status TEXT NOT NULL DEFAULT 'active',
                    repo_path TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "create employees table",
            sql: r#"
                CREATE TABLE employees (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    role TEXT NOT NULL,
                    model TEXT NOT NULL DEFAULT 'gpt-4',
                    status TEXT NOT NULL DEFAULT 'offline',
                    specialization TEXT,
                    system_prompt TEXT,
                    project_id TEXT REFERENCES projects(id),
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "create tasks table",
            sql: r#"
                CREATE TABLE tasks (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    description TEXT,
                    status TEXT NOT NULL DEFAULT 'todo',
                    priority TEXT NOT NULL DEFAULT 'medium',
                    project_id TEXT NOT NULL REFERENCES projects(id),
                    assignee_id TEXT REFERENCES employees(id),
                    complexity INTEGER,
                    ai_suggestion TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "create subtasks table",
            sql: r#"
                CREATE TABLE subtasks (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'todo',
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "create comments table",
            sql: r#"
                CREATE TABLE comments (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    employee_id TEXT REFERENCES employees(id),
                    content TEXT NOT NULL,
                    is_ai_generated INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "create activity_logs table",
            sql: r#"
                CREATE TABLE activity_logs (
                    id TEXT PRIMARY KEY,
                    employee_id TEXT REFERENCES employees(id),
                    action TEXT NOT NULL,
                    details TEXT,
                    task_id TEXT REFERENCES tasks(id),
                    project_id TEXT REFERENCES projects(id),
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "create employee_metrics table",
            sql: r#"
                CREATE TABLE employee_metrics (
                    id TEXT PRIMARY KEY,
                    employee_id TEXT NOT NULL REFERENCES employees(id),
                    tasks_completed INTEGER NOT NULL DEFAULT 0,
                    average_completion_time REAL,
                    success_rate REAL,
                    period_start TEXT NOT NULL,
                    period_end TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 8,
            description: "create project_employees table",
            sql: r#"
                CREATE TABLE project_employees (
                    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    employee_id TEXT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
                    role TEXT NOT NULL DEFAULT 'member',
                    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
                    PRIMARY KEY (project_id, employee_id)
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 9,
            description: "create indexes",
            sql: r#"
                CREATE INDEX idx_tasks_project ON tasks(project_id);
                CREATE INDEX idx_tasks_assignee ON tasks(assignee_id);
                CREATE INDEX idx_tasks_status ON tasks(status);
                CREATE INDEX idx_subtasks_task ON subtasks(task_id);
                CREATE INDEX idx_comments_task ON comments(task_id);
                CREATE INDEX idx_activity_employee ON activity_logs(employee_id);
                CREATE INDEX idx_activity_task ON activity_logs(task_id);
                CREATE INDEX idx_metrics_employee ON employee_metrics(employee_id);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 10,
            description: "create updated_at triggers",
            sql: r#"
                CREATE TRIGGER update_projects_updated_at AFTER UPDATE ON projects
                    FOR EACH ROW BEGIN UPDATE projects SET updated_at = datetime('now') WHERE id = NEW.id; END;
                CREATE TRIGGER update_employees_updated_at AFTER UPDATE ON employees
                    FOR EACH ROW BEGIN UPDATE employees SET updated_at = datetime('now') WHERE id = NEW.id; END;
                CREATE TRIGGER update_tasks_updated_at AFTER UPDATE ON tasks
                    FOR EACH ROW BEGIN UPDATE tasks SET updated_at = datetime('now') WHERE id = NEW.id; END;
                CREATE TRIGGER update_subtasks_updated_at AFTER UPDATE ON subtasks
                    FOR EACH ROW BEGIN UPDATE subtasks SET updated_at = datetime('now') WHERE id = NEW.id; END;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 11,
            description: "insert seed data",
            sql: r#"
                INSERT OR IGNORE INTO projects (id, name, description, status) VALUES
                    ('seed-proj-1', 'Tauri App 开发', '开发跨平台桌面应用', 'active'),
                    ('seed-proj-2', '后端API重构', '重构现有API服务', 'active'),
                    ('seed-proj-3', '文档编写', '编写用户和技术文档', 'active');

                INSERT OR IGNORE INTO employees (id, name, role, model, status, specialization, system_prompt) VALUES
                    ('seed-emp-1', 'Alice Developer', 'developer', 'gpt-4', 'offline', '全栈开发', '你是一个专业的代码开发AI助手'),
                    ('seed-emp-2', 'Bob Reviewer', 'reviewer', 'gpt-4', 'offline', '代码审查', '你是一个代码审查专家'),
                    ('seed-emp-3', 'Carol Tester', 'tester', 'gpt-4', 'offline', '测试工程', '你是一个测试工程师'),
                    ('seed-emp-4', 'Dave Coordinator', 'coordinator', 'gpt-4', 'offline', '项目协调', '你是一个项目协调员');

                INSERT OR IGNORE INTO project_employees (project_id, employee_id, role) VALUES
                    ('seed-proj-1', 'seed-emp-1', 'member'),
                    ('seed-proj-1', 'seed-emp-2', 'member'),
                    ('seed-proj-2', 'seed-emp-1', 'member'),
                    ('seed-proj-2', 'seed-emp-3', 'member'),
                    ('seed-proj-3', 'seed-emp-4', 'member');

                INSERT OR IGNORE INTO tasks (id, title, description, status, priority, project_id, assignee_id) VALUES
                    ('seed-task-1', '实现数据库模块', '设计并实现SQLite数据库架构', 'todo', 'high', 'seed-proj-1', 'seed-emp-1'),
                    ('seed-task-2', '搭建前端框架', '配置React+TailwindCSS+shadcn/ui', 'in_progress', 'high', 'seed-proj-1', 'seed-emp-1'),
                    ('seed-task-3', '代码审查规范', '制定代码审查流程和规范', 'todo', 'medium', 'seed-proj-1', 'seed-emp-2'),
                    ('seed-task-4', 'API接口设计', '设计RESTful API接口', 'review', 'high', 'seed-proj-2', 'seed-emp-1'),
                    ('seed-task-5', '编写测试用例', '为API编写自动化测试', 'todo', 'medium', 'seed-proj-2', 'seed-emp-3');
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 12,
            description: "add reasoning effort and normalize employee model settings",
            sql: r#"
                ALTER TABLE employees ADD COLUMN reasoning_effort TEXT NOT NULL DEFAULT 'high';

                UPDATE employees
                SET reasoning_effort = 'high'
                WHERE reasoning_effort IS NULL
                   OR reasoning_effort NOT IN ('low', 'medium', 'high', 'xhigh');

                UPDATE employees
                SET model = 'gpt-5.4'
                WHERE model IS NULL
                   OR model NOT IN ('gpt-5.4', 'gpt-5.4-mini', 'gpt-5.3-codex', 'gpt-5.2');
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 13,
            description: "track last codex session id on tasks",
            sql: r#"
                ALTER TABLE tasks ADD COLUMN last_codex_session_id TEXT;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
    ]
}
