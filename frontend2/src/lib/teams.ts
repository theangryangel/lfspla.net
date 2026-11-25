export const disciplineLabels = {
  race: "Race",
  drift: "Drift",
  drag: "Drag",
} as const;

/** Legacy team websites sometimes omit their scheme. Only expose web links. */
export function teamWebsite(
  value: string | null | undefined,
): string | undefined {
  if (!value?.trim()) return undefined;
  try {
    const input = value.trim();
    const url = new URL(
      /^[a-z][a-z\d+.-]*:/i.test(input) ? input : `https://${input}`,
    );
    return ["http:", "https:"].includes(url.protocol) &&
      url.hostname &&
      !url.username &&
      !url.password
      ? url.href
      : undefined;
  } catch {
    return undefined;
  }
}

export type TeamFormValues = {
  name: string;
  tag: string;
  description: string;
  website_url: string;
  tags: import("#lib/api.js").Tag[];
  accepts_applications: boolean;
};

export function teamForm(
  team?: import("#lib/api.js").TeamSummary,
): TeamFormValues {
  return {
    name: team?.name ?? "",
    tag: team?.tag ?? "",
    description: team?.description ?? "",
    website_url: team?.website_url ?? "",
    tags: [...(team?.tags ?? [])],
    accepts_applications: team?.accepts_applications ?? false,
  };
}

export function teamDetails(
  values: TeamFormValues,
): import("#lib/api.js").TeamDetailsRequest {
  return {
    ...values,
    tag: values.tag.trim() || null,
    description: values.description.trim() || null,
    website_url: values.website_url.trim() || null,
  };
}
