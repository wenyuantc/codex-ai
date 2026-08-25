import { describe, expect, it } from "vitest";

import { PLAN_QUESTION_OTHER, resolvePlanQuestionAnswer } from "./nativePlanQuestion";

describe("resolvePlanQuestionAnswer", () => {
  const options = ["小程序登录", "公众号登录"];

  it("accepts a provided option", () => {
    expect(resolvePlanQuestionAnswer(options, "小程序登录", "")).toBe("小程序登录");
  });

  it("requires custom text when other is selected", () => {
    expect(resolvePlanQuestionAnswer(options, PLAN_QUESTION_OTHER, "")).toBeNull();
    expect(resolvePlanQuestionAnswer(options, PLAN_QUESTION_OTHER, "  两种都要  ")).toBe(
      "两种都要",
    );
  });

  it("uses free text when there are no options", () => {
    expect(resolvePlanQuestionAnswer([], "", "自定义")).toBe("自定义");
    expect(resolvePlanQuestionAnswer(["only"], "", "   ")).toBeNull();
  });
});
