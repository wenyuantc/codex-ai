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
        Migration {
            version: 14,
            description: "create codex sessions tables",
            sql: r#"
                CREATE TABLE codex_sessions (
                    id TEXT PRIMARY KEY,
                    employee_id TEXT REFERENCES employees(id) ON DELETE SET NULL,
                    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                    cli_session_id TEXT,
                    working_dir TEXT,
                    status TEXT NOT NULL DEFAULT 'pending',
                    started_at TEXT NOT NULL DEFAULT (datetime('now')),
                    ended_at TEXT,
                    exit_code INTEGER,
                    resume_session_id TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE codex_session_events (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES codex_sessions(id) ON DELETE CASCADE,
                    event_type TEXT NOT NULL,
                    message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 15,
            description: "create codex session indexes",
            sql: r#"
                CREATE INDEX idx_codex_sessions_employee_started ON codex_sessions(employee_id, started_at DESC);
                CREATE INDEX idx_codex_sessions_status ON codex_sessions(status);
                CREATE INDEX idx_codex_events_session_created ON codex_session_events(session_id, created_at);
                CREATE INDEX idx_employees_project_id ON employees(project_id);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 16,
            description: "backfill employee project ownership from project employees",
            sql: r#"
                UPDATE employees
                SET project_id = (
                    SELECT pe.project_id
                    FROM project_employees pe
                    WHERE pe.employee_id = employees.id
                    ORDER BY pe.joined_at DESC, pe.project_id ASC
                    LIMIT 1
                )
                WHERE (project_id IS NULL OR project_id = '')
                  AND EXISTS (
                    SELECT 1
                    FROM project_employees pe
                    WHERE pe.employee_id = employees.id
                  );

                INSERT INTO activity_logs (id, employee_id, action, details, created_at)
                SELECT
                    lower(hex(randomblob(16))),
                    employee_id,
                    'employee_project_membership_conflict_migrated',
                    '检测到多项目归属，迁移时保留 joined_at 最新且 project_id 最小的一条关联',
                    datetime('now')
                FROM project_employees
                GROUP BY employee_id
                HAVING COUNT(*) > 1;

                DELETE FROM project_employees;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 17,
            description: "create task attachments table",
            sql: r#"
                CREATE TABLE task_attachments (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    original_name TEXT NOT NULL,
                    stored_path TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    sort_order INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_task_attachments_task_sort
                    ON task_attachments(task_id, sort_order, created_at);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 18,
            description: "add review session and reviewer fields",
            sql: r#"
                ALTER TABLE tasks ADD COLUMN reviewer_id TEXT REFERENCES employees(id);
                ALTER TABLE tasks ADD COLUMN last_review_session_id TEXT;
                ALTER TABLE codex_sessions ADD COLUMN session_kind TEXT NOT NULL DEFAULT 'execution';

                UPDATE codex_sessions
                SET session_kind = 'execution'
                WHERE session_kind IS NULL
                   OR session_kind NOT IN ('execution', 'review');

                CREATE INDEX idx_tasks_reviewer ON tasks(reviewer_id);
                CREATE INDEX idx_codex_sessions_task_kind_started
                    ON codex_sessions(task_id, session_kind, started_at DESC);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 19,
            description: "create codex session file changes table",
            sql: r#"
                CREATE TABLE codex_session_file_changes (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES codex_sessions(id) ON DELETE CASCADE,
                    path TEXT NOT NULL,
                    change_type TEXT NOT NULL,
                    previous_path TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_codex_session_file_changes_session_created
                    ON codex_session_file_changes(session_id, created_at);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 20,
            description: "add capture mode to codex session file changes",
            sql: r#"
                ALTER TABLE codex_session_file_changes
                    ADD COLUMN capture_mode TEXT NOT NULL DEFAULT 'git_fallback';

                UPDATE codex_session_file_changes
                SET capture_mode = 'git_fallback'
                WHERE capture_mode IS NULL OR trim(capture_mode) = '';
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 21,
            description: "create codex session file change details table",
            sql: r#"
                CREATE TABLE codex_session_file_change_details (
                    id TEXT PRIMARY KEY,
                    change_id TEXT NOT NULL UNIQUE REFERENCES codex_session_file_changes(id) ON DELETE CASCADE,
                    absolute_path TEXT,
                    previous_absolute_path TEXT,
                    before_status TEXT NOT NULL DEFAULT 'missing',
                    before_text TEXT,
                    before_truncated INTEGER NOT NULL DEFAULT 0,
                    after_status TEXT NOT NULL DEFAULT 'missing',
                    after_text TEXT,
                    after_truncated INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_codex_session_file_change_details_change
                    ON codex_session_file_change_details(change_id);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 22,
            description: "add task automation configuration and runtime state",
            sql: r#"
                ALTER TABLE tasks ADD COLUMN automation_mode TEXT;

                CREATE TABLE task_automation_state (
                    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
                    phase TEXT NOT NULL DEFAULT 'idle',
                    round_count INTEGER NOT NULL DEFAULT 0,
                    consumed_session_id TEXT,
                    last_trigger_session_id TEXT,
                    pending_action TEXT,
                    pending_round_count INTEGER,
                    last_error TEXT,
                    last_verdict_json TEXT,
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_task_automation_state_phase
                    ON task_automation_state(phase, updated_at);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 23,
            description: "add ssh projects runtime profiles and remote session metadata",
            sql: r#"
                CREATE TABLE ssh_configs (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    host TEXT NOT NULL,
                    port INTEGER NOT NULL DEFAULT 22,
                    username TEXT NOT NULL,
                    auth_type TEXT NOT NULL DEFAULT 'key' CHECK (auth_type IN ('key', 'password')),
                    private_key_path TEXT,
                    password_ref TEXT,
                    passphrase_ref TEXT,
                    known_hosts_mode TEXT NOT NULL DEFAULT 'accept-new',
                    last_checked_at TEXT,
                    last_check_status TEXT,
                    last_check_message TEXT,
                    password_probe_checked_at TEXT,
                    password_probe_status TEXT,
                    password_probe_message TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_ssh_configs_name ON ssh_configs(name);
                CREATE INDEX idx_ssh_configs_host ON ssh_configs(host, username);

                CREATE TRIGGER update_ssh_configs_updated_at AFTER UPDATE ON ssh_configs
                    FOR EACH ROW BEGIN UPDATE ssh_configs SET updated_at = datetime('now') WHERE id = NEW.id; END;

                ALTER TABLE projects
                    ADD COLUMN project_type TEXT NOT NULL DEFAULT 'local'
                    CHECK (project_type IN ('local', 'ssh'));
                ALTER TABLE projects
                    ADD COLUMN ssh_config_id TEXT REFERENCES ssh_configs(id) ON DELETE RESTRICT;
                ALTER TABLE projects
                    ADD COLUMN remote_repo_path TEXT;

                UPDATE projects
                SET project_type = 'local'
                WHERE project_type IS NULL
                   OR project_type NOT IN ('local', 'ssh');

                CREATE INDEX idx_projects_project_type ON projects(project_type, updated_at DESC);
                CREATE INDEX idx_projects_ssh_config_id ON projects(ssh_config_id);

                ALTER TABLE codex_sessions
                    ADD COLUMN execution_target TEXT NOT NULL DEFAULT 'local'
                    CHECK (execution_target IN ('local', 'ssh'));
                ALTER TABLE codex_sessions
                    ADD COLUMN ssh_config_id TEXT REFERENCES ssh_configs(id) ON DELETE SET NULL;
                ALTER TABLE codex_sessions
                    ADD COLUMN target_host_label TEXT;
                ALTER TABLE codex_sessions
                    ADD COLUMN artifact_capture_mode TEXT NOT NULL DEFAULT 'local_full'
                    CHECK (artifact_capture_mode IN ('local_full', 'ssh_git_status', 'ssh_none'));

                UPDATE codex_sessions
                SET execution_target = 'local'
                WHERE execution_target IS NULL
                   OR execution_target NOT IN ('local', 'ssh');

                UPDATE codex_sessions
                SET artifact_capture_mode = 'local_full'
                WHERE artifact_capture_mode IS NULL
                   OR artifact_capture_mode NOT IN ('local_full', 'ssh_git_status', 'ssh_none');

                CREATE INDEX idx_codex_sessions_execution_target_started
                    ON codex_sessions(execution_target, started_at DESC);
                CREATE INDEX idx_codex_sessions_ssh_config_id
                    ON codex_sessions(ssh_config_id, started_at DESC);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 24,
            description: "allow full ssh artifact capture mode for codex sessions",
            sql: r#"
                PRAGMA foreign_keys=OFF;

                CREATE TABLE codex_sessions_new (
                    id TEXT PRIMARY KEY,
                    employee_id TEXT REFERENCES employees(id) ON DELETE SET NULL,
                    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                    cli_session_id TEXT,
                    working_dir TEXT,
                    status TEXT NOT NULL DEFAULT 'pending',
                    started_at TEXT NOT NULL DEFAULT (datetime('now')),
                    ended_at TEXT,
                    exit_code INTEGER,
                    resume_session_id TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    session_kind TEXT NOT NULL DEFAULT 'execution',
                    execution_target TEXT NOT NULL DEFAULT 'local'
                        CHECK (execution_target IN ('local', 'ssh')),
                    ssh_config_id TEXT REFERENCES ssh_configs(id) ON DELETE SET NULL,
                    target_host_label TEXT,
                    artifact_capture_mode TEXT NOT NULL DEFAULT 'local_full'
                        CHECK (artifact_capture_mode IN ('local_full', 'ssh_full', 'ssh_git_status', 'ssh_none'))
                );

                INSERT INTO codex_sessions_new (
                    id,
                    employee_id,
                    task_id,
                    project_id,
                    cli_session_id,
                    working_dir,
                    status,
                    started_at,
                    ended_at,
                    exit_code,
                    resume_session_id,
                    created_at,
                    session_kind,
                    execution_target,
                    ssh_config_id,
                    target_host_label,
                    artifact_capture_mode
                )
                SELECT
                    id,
                    employee_id,
                    task_id,
                    project_id,
                    cli_session_id,
                    working_dir,
                    status,
                    started_at,
                    ended_at,
                    exit_code,
                    resume_session_id,
                    created_at,
                    session_kind,
                    execution_target,
                    ssh_config_id,
                    target_host_label,
                    CASE
                        WHEN artifact_capture_mode IN ('local_full', 'ssh_git_status', 'ssh_none')
                            THEN artifact_capture_mode
                        ELSE 'local_full'
                    END
                FROM codex_sessions;

                DROP TABLE codex_sessions;
                ALTER TABLE codex_sessions_new RENAME TO codex_sessions;

                CREATE INDEX idx_codex_sessions_employee_started
                    ON codex_sessions(employee_id, started_at DESC);
                CREATE INDEX idx_codex_sessions_status
                    ON codex_sessions(status);
                CREATE INDEX idx_codex_sessions_task_kind_started
                    ON codex_sessions(task_id, session_kind, started_at DESC);
                CREATE INDEX idx_codex_sessions_execution_target_started
                    ON codex_sessions(execution_target, started_at DESC);
                CREATE INDEX idx_codex_sessions_ssh_config_id
                    ON codex_sessions(ssh_config_id, started_at DESC);

                PRAGMA foreign_keys=ON;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 25,
            description: "create task git contexts and link codex sessions",
            sql: r#"
                CREATE TABLE task_git_contexts (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
                    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                    base_branch TEXT,
                    task_branch TEXT,
                    target_branch TEXT,
                    worktree_path TEXT,
                    repo_head_commit_at_prepare TEXT,
                    state TEXT NOT NULL DEFAULT 'provisioning'
                        CHECK (state IN (
                            'provisioning',
                            'ready',
                            'running',
                            'merge_ready',
                            'action_pending',
                            'completed',
                            'failed',
                            'drifted'
                        )),
                    context_version INTEGER NOT NULL DEFAULT 1
                        CHECK (context_version >= 1),
                    pending_action_type TEXT
                        CHECK (
                            pending_action_type IS NULL
                            OR pending_action_type IN (
                                'merge',
                                'push',
                                'rebase',
                                'cherry_pick',
                                'stash',
                                'unstash',
                                'cleanup_worktree'
                            )
                        ),
                    pending_action_token_hash TEXT,
                    pending_action_payload_json TEXT,
                    pending_action_nonce TEXT,
                    pending_action_requested_at TEXT,
                    pending_action_expires_at TEXT,
                    pending_action_repo_revision TEXT,
                    pending_action_bound_context_version INTEGER
                        CHECK (
                            pending_action_bound_context_version IS NULL
                            OR pending_action_bound_context_version >= 1
                        ),
                    last_reconciled_at TEXT,
                    last_error TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_task_git_contexts_project_updated
                    ON task_git_contexts(project_id, updated_at DESC);
                CREATE INDEX idx_task_git_contexts_state_updated
                    ON task_git_contexts(state, updated_at DESC);
                CREATE INDEX idx_task_git_contexts_pending_expires
                    ON task_git_contexts(pending_action_expires_at);

                CREATE TRIGGER update_task_git_contexts_updated_at AFTER UPDATE ON task_git_contexts
                    FOR EACH ROW BEGIN UPDATE task_git_contexts SET updated_at = datetime('now') WHERE id = NEW.id; END;

                ALTER TABLE codex_sessions
                    ADD COLUMN task_git_context_id TEXT REFERENCES task_git_contexts(id) ON DELETE SET NULL;

                CREATE INDEX idx_codex_sessions_task_git_context_started
                    ON codex_sessions(task_git_context_id, started_at DESC);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 26,
            description: "normalize employee models to latest supported codex list",
            sql: r#"
                UPDATE employees
                SET model = 'gpt-5.4'
                WHERE model IS NULL
                   OR model NOT IN (
                     'gpt-5.4',
                     'gpt-5.2-codex',
                     'gpt-5.1-codex-max',
                     'gpt-5.4-mini',
                     'gpt-5.3-codex',
                     'gpt-5.3-codex-spark',
                     'gpt-5.2',
                     'gpt-5.1-codex-mini'
                   );
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 27,
            description: "persist task worktree mode preference",
            sql: r#"
                ALTER TABLE tasks
                    ADD COLUMN use_worktree INTEGER NOT NULL DEFAULT 1
                    CHECK (use_worktree IN (0, 1));
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 28,
            description: "create notification center table",
            sql: r#"
                CREATE TABLE notifications (
                    id TEXT PRIMARY KEY,
                    notification_type TEXT NOT NULL
                        CHECK (
                            notification_type IN (
                                'review_pending',
                                'run_failed',
                                'task_completed',
                                'sdk_unavailable',
                                'database_error',
                                'ssh_config_error'
                            )
                        ),
                    severity TEXT NOT NULL
                        CHECK (severity IN ('info', 'success', 'warning', 'error', 'critical')),
                    source_module TEXT NOT NULL,
                    title TEXT NOT NULL,
                    message TEXT NOT NULL,
                    recommendation TEXT,
                    action_label TEXT,
                    action_route TEXT,
                    related_object_type TEXT,
                    related_object_id TEXT,
                    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                    ssh_config_id TEXT REFERENCES ssh_configs(id) ON DELETE SET NULL,
                    delivery_mode TEXT NOT NULL DEFAULT 'one_time'
                        CHECK (delivery_mode IN ('one_time', 'sticky')),
                    state TEXT NOT NULL DEFAULT 'active'
                        CHECK (state IN ('active', 'resolved')),
                    is_read INTEGER NOT NULL DEFAULT 0 CHECK (is_read IN (0, 1)),
                    dedupe_key TEXT,
                    occurrence_count INTEGER NOT NULL DEFAULT 1 CHECK (occurrence_count >= 1),
                    first_triggered_at TEXT NOT NULL DEFAULT (datetime('now')),
                    last_triggered_at TEXT NOT NULL DEFAULT (datetime('now')),
                    read_at TEXT,
                    resolved_at TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX idx_notifications_last_triggered
                    ON notifications(state, is_read, last_triggered_at DESC);
                CREATE INDEX idx_notifications_project
                    ON notifications(project_id, last_triggered_at DESC);
                CREATE INDEX idx_notifications_task
                    ON notifications(task_id, last_triggered_at DESC);
                CREATE INDEX idx_notifications_ssh_config
                    ON notifications(ssh_config_id, last_triggered_at DESC);
                CREATE UNIQUE INDEX idx_notifications_active_dedupe
                    ON notifications(dedupe_key)
                    WHERE dedupe_key IS NOT NULL AND state = 'active';

                CREATE TRIGGER update_notifications_updated_at AFTER UPDATE ON notifications
                    FOR EACH ROW BEGIN UPDATE notifications SET updated_at = datetime('now') WHERE id = NEW.id; END;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 29,
            description: "add run completed notification type",
            sql: r#"
                ALTER TABLE notifications RENAME TO notifications_old;

                CREATE TABLE notifications (
                    id TEXT PRIMARY KEY,
                    notification_type TEXT NOT NULL
                        CHECK (
                            notification_type IN (
                                'review_pending',
                                'run_failed',
                                'run_completed',
                                'task_completed',
                                'sdk_unavailable',
                                'database_error',
                                'ssh_config_error'
                            )
                        ),
                    severity TEXT NOT NULL
                        CHECK (severity IN ('info', 'success', 'warning', 'error', 'critical')),
                    source_module TEXT NOT NULL,
                    title TEXT NOT NULL,
                    message TEXT NOT NULL,
                    recommendation TEXT,
                    action_label TEXT,
                    action_route TEXT,
                    related_object_type TEXT,
                    related_object_id TEXT,
                    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                    ssh_config_id TEXT REFERENCES ssh_configs(id) ON DELETE SET NULL,
                    delivery_mode TEXT NOT NULL DEFAULT 'one_time'
                        CHECK (delivery_mode IN ('one_time', 'sticky')),
                    state TEXT NOT NULL DEFAULT 'active'
                        CHECK (state IN ('active', 'resolved')),
                    is_read INTEGER NOT NULL DEFAULT 0 CHECK (is_read IN (0, 1)),
                    dedupe_key TEXT,
                    occurrence_count INTEGER NOT NULL DEFAULT 1 CHECK (occurrence_count >= 1),
                    first_triggered_at TEXT NOT NULL DEFAULT (datetime('now')),
                    last_triggered_at TEXT NOT NULL DEFAULT (datetime('now')),
                    read_at TEXT,
                    resolved_at TEXT,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                );

                INSERT INTO notifications (
                    id,
                    notification_type,
                    severity,
                    source_module,
                    title,
                    message,
                    recommendation,
                    action_label,
                    action_route,
                    related_object_type,
                    related_object_id,
                    project_id,
                    task_id,
                    ssh_config_id,
                    delivery_mode,
                    state,
                    is_read,
                    dedupe_key,
                    occurrence_count,
                    first_triggered_at,
                    last_triggered_at,
                    read_at,
                    resolved_at,
                    created_at,
                    updated_at
                )
                SELECT
                    id,
                    notification_type,
                    severity,
                    source_module,
                    title,
                    message,
                    recommendation,
                    action_label,
                    action_route,
                    related_object_type,
                    related_object_id,
                    project_id,
                    task_id,
                    ssh_config_id,
                    delivery_mode,
                    state,
                    is_read,
                    dedupe_key,
                    occurrence_count,
                    first_triggered_at,
                    last_triggered_at,
                    read_at,
                    resolved_at,
                    created_at,
                    updated_at
                FROM notifications_old;

                DROP TABLE notifications_old;

                CREATE INDEX idx_notifications_last_triggered
                    ON notifications(state, is_read, last_triggered_at DESC);
                CREATE INDEX idx_notifications_project
                    ON notifications(project_id, last_triggered_at DESC);
                CREATE INDEX idx_notifications_task
                    ON notifications(task_id, last_triggered_at DESC);
                CREATE INDEX idx_notifications_ssh_config
                    ON notifications(ssh_config_id, last_triggered_at DESC);
                CREATE UNIQUE INDEX idx_notifications_active_dedupe
                    ON notifications(dedupe_key)
                    WHERE dedupe_key IS NOT NULL AND state = 'active';

                CREATE TRIGGER update_notifications_updated_at AFTER UPDATE ON notifications
                    FOR EACH ROW BEGIN UPDATE notifications SET updated_at = datetime('now') WHERE id = NEW.id; END;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 30,
            description: "add archived task management index",
            sql: r#"
                CREATE INDEX idx_tasks_project_status_updated
                    ON tasks(project_id, status, updated_at DESC);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 31,
            description: "add ai_provider column to employees",
            sql: r#"
                ALTER TABLE employees ADD COLUMN ai_provider TEXT NOT NULL DEFAULT 'codex';
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 32,
            description: "add ai_provider column to codex_sessions",
            sql: r#"
                ALTER TABLE codex_sessions ADD COLUMN ai_provider TEXT NOT NULL DEFAULT 'codex';
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 33,
            description: "add thinking_budget_tokens column to codex_sessions for Claude",
            sql: r#"
                ALTER TABLE codex_sessions ADD COLUMN thinking_budget_tokens INTEGER;
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 34,
            description: "persist task coordinator and plan content",
            sql: r#"
                ALTER TABLE tasks
                    ADD COLUMN coordinator_id TEXT REFERENCES employees(id) ON DELETE SET NULL;
                ALTER TABLE tasks
                    ADD COLUMN plan_content TEXT;

                CREATE INDEX idx_tasks_coordinator ON tasks(coordinator_id);
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
        Migration {
            version: 35,
            description: "enhance default employee system prompts",
            sql: r#"
                UPDATE employees
                SET system_prompt = '你是负责实际交付的开发员工。执行任务前先阅读仓库说明、相关代码和现有实现，复用项目已有模式与工具，保持改动小而可回滚。实现时特别注意数据库变更必须包含迁移脚本，时间展示走 formatDate()，新增功能补充最近活动日志并让仪表盘 key 显示中文，兼容本地与 SSH 项目，大文本编辑和预览使用 Monaco 编辑器。完成后运行必要的 lint、构建、测试或说明未验证风险。'
                WHERE id = 'seed-emp-1'
                  AND system_prompt = '你是一个专业的代码开发AI助手';

                UPDATE employees
                SET system_prompt = '你是严格的代码审查员工。按严重程度优先指出行为回归、数据安全/迁移风险、SSH 兼容性、任务自动化链路风险、缺失测试和可维护性问题。每条意见应给出具体文件与行号、触发条件、用户影响和建议修复；没有阻断问题时也要说明剩余风险与验证缺口。'
                WHERE id = 'seed-emp-2'
                  AND system_prompt = '你是一个代码审查专家';

                UPDATE employees
                SET system_prompt = '你是以验收为导向的测试员工。从任务目标和现有行为出发设计验证场景，覆盖正常流程、边界输入、失败恢复、数据库迁移、SSH 项目兼容性、最近活动日志、时间格式和 UI 可用性。优先选择能证明风险已关闭的自动化测试、构建检查和手工烟测，并清楚记录已测项、未测项与失败证据。'
                WHERE id = 'seed-emp-3'
                  AND system_prompt = '你是一个测试工程师';

                UPDATE employees
                SET system_prompt = '你是协调任务落地的项目员工。先澄清目标、范围、依赖和验收标准，再把工作拆成可执行步骤，标明负责人角色、顺序、共享文件风险和验证要求。推进过程中保持开发、审查、测试闭环，发现阻塞时给出替代路径，交付时汇总完成内容、证据和剩余风险。'
                WHERE id = 'seed-emp-4'
                  AND system_prompt = '你是一个项目协调员';
            "#,
            kind: tauri_plugin_sql::MigrationKind::Up,
        },
    ]
}

pub fn latest_migration_version() -> i64 {
    get_all_migrations()
        .last()
        .map(|migration| migration.version)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use sqlx::{Row, SqlitePool};

    use super::{get_all_migrations, latest_migration_version};

    async fn setup_test_pool() -> SqlitePool {
        setup_test_pool_through(latest_migration_version()).await
    }

    async fn setup_test_pool_through(max_version: i64) -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("create sqlite memory pool");

        for migration in get_all_migrations() {
            if migration.version > max_version {
                continue;
            }

            sqlx::raw_sql(migration.sql)
                .execute(&pool)
                .await
                .unwrap_or_else(|error| panic!("run migration {}: {}", migration.version, error));
        }

        pool
    }

    #[test]
    fn latest_migration_version_includes_default_employee_prompt_enhancement() {
        assert_eq!(latest_migration_version(), 35);
    }

    #[test]
    fn migration_versions_are_contiguous() {
        for (index, migration) in get_all_migrations().iter().enumerate() {
            assert_eq!(migration.version, index as i64 + 1);
        }
    }

    #[test]
    fn migration_enhances_only_unchanged_default_employee_prompts() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool_through(34).await;

            sqlx::query(
                r#"
                UPDATE employees
                SET system_prompt = CASE id
                    WHEN 'seed-emp-1' THEN '你是一个专业的代码开发AI助手'
                    WHEN 'seed-emp-2' THEN '保留我的自定义审查提示词'
                    WHEN 'seed-emp-3' THEN '你是一个测试工程师'
                    WHEN 'seed-emp-4' THEN '你是一个项目协调员'
                END
                WHERE id IN ('seed-emp-1', 'seed-emp-2', 'seed-emp-3', 'seed-emp-4')
                "#,
            )
            .execute(&pool)
            .await
            .expect("prepare old default employee prompts");

            let migration = get_all_migrations()
                .into_iter()
                .find(|migration| migration.version == 35)
                .expect("find migration 35");
            sqlx::raw_sql(migration.sql)
                .execute(&pool)
                .await
                .expect("run migration 35");

            let prompts = sqlx::query(
                "SELECT id, system_prompt FROM employees WHERE id IN ('seed-emp-1', 'seed-emp-2', 'seed-emp-3', 'seed-emp-4')",
            )
            .fetch_all(&pool)
            .await
            .expect("fetch employee prompts")
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("id"),
                    row.get::<String, _>("system_prompt"),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();

            assert!(prompts["seed-emp-1"].contains("负责实际交付的开发员工"));
            assert_eq!(prompts["seed-emp-2"], "保留我的自定义审查提示词");
            assert!(prompts["seed-emp-3"].contains("以验收为导向的测试员工"));
            assert!(prompts["seed-emp-4"].contains("协调任务落地的项目员工"));
        });
    }

    #[test]
    fn fresh_database_seed_employees_end_with_enhanced_prompts() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;

            let prompts = sqlx::query(
                "SELECT id, system_prompt FROM employees WHERE id IN ('seed-emp-1', 'seed-emp-2', 'seed-emp-3', 'seed-emp-4')",
            )
            .fetch_all(&pool)
            .await
            .expect("fetch seed employee prompts")
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("id"),
                    row.get::<String, _>("system_prompt"),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();

            assert!(prompts["seed-emp-1"].contains("负责实际交付的开发员工"));
            assert!(prompts["seed-emp-2"].contains("严格的代码审查员工"));
            assert!(prompts["seed-emp-3"].contains("以验收为导向的测试员工"));
            assert!(prompts["seed-emp-4"].contains("协调任务落地的项目员工"));
        });
    }

    #[test]
    fn migration_creates_task_git_context_schema() {
        tauri::async_runtime::block_on(async {
            let pool = setup_test_pool().await;

            let table = sqlx::query_scalar::<_, String>(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'task_git_contexts'",
            )
            .fetch_one(&pool)
            .await
            .expect("task_git_contexts table exists");
            assert_eq!(table, "task_git_contexts");

            let columns = sqlx::query("PRAGMA table_info(task_git_contexts)")
                .fetch_all(&pool)
                .await
                .expect("fetch task_git_contexts columns");
            let column_names = columns
                .iter()
                .map(|row| row.get::<String, _>("name"))
                .collect::<Vec<_>>();

            assert!(column_names.contains(&"context_version".to_string()));
            assert!(column_names.contains(&"pending_action_type".to_string()));
            assert!(column_names.contains(&"pending_action_payload_json".to_string()));
            assert!(column_names.contains(&"pending_action_bound_context_version".to_string()));

            let session_columns = sqlx::query("PRAGMA table_info(codex_sessions)")
                .fetch_all(&pool)
                .await
                .expect("fetch codex_sessions columns");
            let session_column_names = session_columns
                .iter()
                .map(|row| row.get::<String, _>("name"))
                .collect::<Vec<_>>();
            assert!(session_column_names.contains(&"task_git_context_id".to_string()));

            let foreign_keys = sqlx::query("PRAGMA foreign_key_list(codex_sessions)")
                .fetch_all(&pool)
                .await
                .expect("fetch codex_sessions foreign keys");
            assert!(foreign_keys.iter().any(|row| {
                row.get::<String, _>("table") == "task_git_contexts"
                    && row.get::<String, _>("from") == "task_git_context_id"
            }));

            let index_names = sqlx::query(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'task_git_contexts'",
            )
            .fetch_all(&pool)
            .await
            .expect("fetch task_git_contexts indexes")
            .into_iter()
            .map(|row| row.get::<String, _>("name"))
            .collect::<Vec<_>>();

            assert!(index_names.contains(&"idx_task_git_contexts_project_updated".to_string()));
            assert!(index_names.contains(&"idx_task_git_contexts_state_updated".to_string()));
        });
    }
}
