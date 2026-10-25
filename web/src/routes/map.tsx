import { createFileRoute } from "@tanstack/react-router";
import { MapPage, validateMapSearch } from "@/components/map/MapPage";

export const Route = createFileRoute("/map")({
  validateSearch: validateMapSearch,
  component: function MapRoute() {
    return <MapPage search={Route.useSearch()} />;
  },
});
