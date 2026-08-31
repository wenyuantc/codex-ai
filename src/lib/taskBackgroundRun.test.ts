import { describe, expect, it } from "vitest";

import {
  canOfferTaskBackgroundRun,
  isTaskBackgroundRunDisabled,
  resolveTaskBackgroundRunInitialPhase,
  shouldGenerateCoordinatorPlanForBackgroundRun,
} from "@/lib/taskBackgroundRunPolicy";

describe("canOfferTaskBackgroundRun", () => {
  it("offers the action only for the run CTA and not on the completed column", () => {
    expect(
      canOfferTaskBackgroundRun({
        primaryCtaKind: "run",
      }),
    ).toBe(true);
    expect(
      canOfferTaskBackgroundRun({
        primaryCtaKind: "run",
        hideRunAction: true,
      }),
    ).toBe(false);
    expect(
      canOfferTaskBackgroundRun({
        primaryCtaKind: "stop",
      }),
    ).toBe(false);
    expect(
      canOfferTaskBackgroundRun({
        primaryCtaKind: "queued",
      }),
    ).toBe(false);
    expect(
      canOfferTaskBackgroundRun({
        primaryCtaKind: "starting",
      }),
    ).toBe(false);
  });
});

describe("isTaskBackgroundRunDisabled", () => {
  it("disables when the primary run CTA cannot start", () => {
    const idle = {
      canOffer: true,
      primaryCtaDisabled: false,
      isActionLoading: false,
      isRunning: false,
      isReviewRunning: false,
    };
    expect(isTaskBackgroundRunDisabled(idle)).toBe(false);
    expect(isTaskBackgroundRunDisabled({ ...idle, primaryCtaDisabled: true })).toBe(true);
    expect(isTaskBackgroundRunDisabled({ ...idle, isActionLoading: true })).toBe(true);
    expect(isTaskBackgroundRunDisabled({ ...idle, isRunning: true })).toBe(true);
    expect(isTaskBackgroundRunDisabled({ ...idle, isReviewRunning: true })).toBe(true);
    expect(isTaskBackgroundRunDisabled({ ...idle, canOffer: false })).toBe(true);
  });
});

describe("shouldGenerateCoordinatorPlanForBackgroundRun", () => {
  it("reuses a saved coordinator plan and generates only when missing", () => {
    expect(
      shouldGenerateCoordinatorPlanForBackgroundRun({
        coordinatorId: "c1",
        savedPlan: "已有计划",
      }),
    ).toBe(false);
    expect(
      shouldGenerateCoordinatorPlanForBackgroundRun({
        coordinatorId: "c1",
        savedPlan: "  ",
      }),
    ).toBe(true);
    expect(
      shouldGenerateCoordinatorPlanForBackgroundRun({
        coordinatorId: null,
        savedPlan: "",
      }),
    ).toBe(false);
  });

  it("starts in planning only when a coordinator plan must be generated", () => {
    expect(
      resolveTaskBackgroundRunInitialPhase({
        coordinatorId: "c1",
        savedPlan: null,
      }),
    ).toBe("planning");
    expect(
      resolveTaskBackgroundRunInitialPhase({
        coordinatorId: "c1",
        savedPlan: "目标与范围",
      }),
    ).toBe("starting");
    expect(
      resolveTaskBackgroundRunInitialPhase({
        coordinatorId: null,
        savedPlan: null,
      }),
    ).toBe("starting");
  });
});
