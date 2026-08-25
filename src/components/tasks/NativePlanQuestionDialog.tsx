import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  answerNativePlanQuestion,
  onNativePlanQuestion,
  type NativePlanQuestionRequest,
} from "@/lib/native";
import { PLAN_QUESTION_OTHER, resolvePlanQuestionAnswer } from "@/lib/nativePlanQuestion";

function optionClassName(active: boolean): string {
  return `rounded-md border px-2 py-1.5 text-left text-sm whitespace-normal break-words ${
    active ? "border-primary bg-primary/10" : "border-input hover:bg-accent"
  }`;
}

export function NativePlanQuestionDialog() {
  const { t } = useTranslation("tasks");
  const [pending, setPending] = useState<NativePlanQuestionRequest | null>(null);
  const [selections, setSelections] = useState<string[]>([]);
  const [otherTexts, setOtherTexts] = useState<string[]>([]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void onNativePlanQuestion((request) => {
      setPending(request);
      setSelections(request.questions.map(() => ""));
      setOtherTexts(request.questions.map(() => ""));
    }).then((fn) => {
      if (cancelled) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const resolvedAnswers =
    pending?.questions.map((question, index) =>
      resolvePlanQuestionAnswer(question.options, selections[index] ?? "", otherTexts[index] ?? ""),
    ) ?? [];

  const submit = (skipped: boolean) => {
    if (!pending) {
      return;
    }
    const current = pending;
    setPending(null);
    void answerNativePlanQuestion(
      current.sessionRecordId,
      current.requestId,
      skipped,
      resolvedAnswers.map((item) => item ?? ""),
    );
  };

  const canSubmit = pending !== null && resolvedAnswers.every((item) => item !== null);

  const setSelection = (index: number, value: string) => {
    setSelections((current) => {
      const next = [...current];
      next[index] = value;
      return next;
    });
  };

  const setOtherText = (index: number, value: string) => {
    setOtherTexts((current) => {
      const next = [...current];
      next[index] = value;
      return next;
    });
  };

  return (
    <Dialog
      open={Boolean(pending)}
      onOpenChange={(next) => {
        if (!next && pending) {
          submit(true);
        }
      }}
    >
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("nativePlanQuestion.title")}</DialogTitle>
          <DialogDescription>{t("nativePlanQuestion.description")}</DialogDescription>
        </DialogHeader>
        <div className="max-h-80 space-y-4 overflow-y-auto pr-1">
          {pending?.questions.map((question, index) => (
            <div key={`${question.prompt}-${index}`} className="space-y-2">
              <p className="text-sm font-medium whitespace-normal break-words">{question.prompt}</p>
              {question.options.length >= 2 ? (
                <div className="flex flex-col gap-1">
                  {question.options.map((option) => (
                    <button
                      key={option}
                      type="button"
                      className={optionClassName(selections[index] === option)}
                      onClick={() => setSelection(index, option)}
                    >
                      {option}
                    </button>
                  ))}
                  <button
                    type="button"
                    className={optionClassName(selections[index] === PLAN_QUESTION_OTHER)}
                    onClick={() => setSelection(index, PLAN_QUESTION_OTHER)}
                  >
                    {t("nativePlanQuestion.other")}
                  </button>
                  {selections[index] === PLAN_QUESTION_OTHER ? (
                    <Input
                      value={otherTexts[index] ?? ""}
                      onChange={(event) => setOtherText(index, event.target.value)}
                      placeholder={t("nativePlanQuestion.otherPlaceholder")}
                    />
                  ) : null}
                </div>
              ) : (
                <Input
                  value={otherTexts[index] ?? ""}
                  onChange={(event) => setOtherText(index, event.target.value)}
                  placeholder={t("nativePlanQuestion.answerPlaceholder")}
                />
              )}
            </div>
          ))}
        </div>
        <DialogFooter className="mt-2 flex-col gap-2 sm:flex-col">
          <Button type="button" disabled={!canSubmit} onClick={() => submit(false)}>
            {t("nativePlanQuestion.submit")}
          </Button>
          <Button type="button" variant="ghost" onClick={() => submit(true)}>
            {t("nativePlanQuestion.skip")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
