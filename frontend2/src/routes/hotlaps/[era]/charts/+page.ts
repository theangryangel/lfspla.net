import { createApi, type HotLapDiscipline } from "#lib/api.js";
import { lastChart, rememberChart } from "#lib/chart-history.js";
import { error, redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types";

export const load: PageLoad = async ({ params, url, fetch }) => {
  // A chart used to be named by the query string. Saved links keep working,
  // permanently redirected to the path that names the same combination.
  const legacyTrack = url.searchParams.get("track");
  const legacyVehicle = url.searchParams.get("vehicle");
  if (legacyTrack && legacyVehicle) {
    const filters = new URLSearchParams(url.searchParams);
    filters.delete("track");
    filters.delete("vehicle");
    const search = filters.toString();
    redirect(
      308,
      `/hotlaps/${encodeURIComponent(params.era)}/charts/${encodeURIComponent(legacyTrack)}/${encodeURIComponent(legacyVehicle)}` +
        (search ? `?${search}` : ""),
    );
  }

  const api = createApi(fetch);
  const value = url.searchParams.get("discipline");
  const discipline: HotLapDiscipline | undefined =
    value === "circuit" || value === "drag" ? value : undefined;
  const suffix = discipline ? `?discipline=${discipline}` : "";
  const candidates = [
    lastChart(params.era),
    discipline ? lastChart(params.era, discipline) : null,
  ];
  for (const remembered of candidates) {
    if (!remembered) continue;
    const offered = await api.tracks.listEraTracks({
      era: params.era,
      vehicle: remembered.vehicle,
      discipline,
    });
    if (
      offered.items.some((candidate) => candidate.code === remembered.track)
    ) {
      rememberChart(params.era, remembered, discipline);
      redirect(
        307,
        `/hotlaps/${encodeURIComponent(params.era)}/charts/${encodeURIComponent(remembered.track)}/${encodeURIComponent(remembered.vehicle)}${suffix}`,
      );
    }
  }
  const tracks = (
    await api.tracks.listEraTracks({ era: params.era, discipline })
  ).items;
  const track =
    tracks.find((candidate) => candidate.code === "BL1") ?? tracks[0];
  if (!track) error(404, "No charts available in this era.");
  const vehicles = (
    await api.vehicles.listEraVehicles({
      era: params.era,
      track: track.code,
      discipline,
      limit: 100,
    })
  ).items;
  const vehicle =
    vehicles.find((candidate) => candidate.code === "XFG") ?? vehicles[0];
  if (!vehicle) error(404, "No vehicles available for this chart.");
  rememberChart(
    params.era,
    { track: track.code, vehicle: vehicle.code },
    discipline,
  );
  redirect(
    307,
    `/hotlaps/${encodeURIComponent(params.era)}/charts/${encodeURIComponent(track.code)}/${encodeURIComponent(vehicle.code)}${suffix}`,
  );
};
