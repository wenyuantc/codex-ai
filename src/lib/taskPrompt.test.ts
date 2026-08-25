import { describe, expect, it } from "vitest";

import { buildTaskExecutionInput } from "@/lib/taskPrompt";

describe("buildTaskExecutionInput", () => {
  it("includes project file refs as relative paths", () => {
    const result = buildTaskExecutionInput({
      title: "修复登录",
      description: "处理记住我",
      fileRefs: ["src/pages/LoginPage.tsx", { relative_path: "src/lib/auth.ts" }],
    });

    expect(result.prompt).toContain("项目文件引用:");
    expect(result.prompt).toContain("1. src/pages/LoginPage.tsx");
    expect(result.prompt).toContain("2. src/lib/auth.ts");
    expect(result.prompt).toContain("相对项目仓库根目录");
  });

  it("omits the file-ref section when no paths are provided", () => {
    const result = buildTaskExecutionInput({
      title: "空引用",
      fileRefs: ["  ", { relative_path: "" }],
    });

    expect(result.prompt).not.toContain("项目文件引用:");
  });
});
