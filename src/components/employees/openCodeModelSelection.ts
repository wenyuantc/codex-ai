import type { OpenCodeModelInfo } from "@/lib/opencode";
import { normalizeReasoningEffortForProvider } from "@/lib/types";

export function selectOpenCodeModel(
  models: OpenCodeModelInfo[],
  currentModel: string,
): string {
  const trimmedModel = currentModel.trim();

  if (models.length === 0) {
    return trimmedModel;
  }

  return models.some((model) => model.value === trimmedModel)
    ? trimmedModel
    : models[0].value;
}

export function selectOpenCodeReasoningEffort(
  models: OpenCodeModelInfo[],
  selectedModel: string,
  currentEffort: string,
): string {
  const modelInfo = models.find((model) => model.value === selectedModel);

  if (modelInfo?.capabilities && !modelInfo.capabilities.reasoning) {
    return "auto";
  }

  return normalizeReasoningEffortForProvider("opencode", currentEffort);
}
