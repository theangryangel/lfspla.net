import type { ReportReason } from "#lib/api.js";

export const reportReasons = {
  suspected_cheating: "Suspected cheating",
  invalid_lap: "Invalid lap",
  incorrect_classification: "Incorrect classification",
  other_problem: "Another problem",
} satisfies Record<ReportReason, string>;
