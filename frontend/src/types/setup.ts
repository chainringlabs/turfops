// Mirrors backend/src/api/setup.rs, backend/src/api/sources.rs (Preview, StationsQuery),
// backend/src/api/sources/view.rs (StationChoice) and
// backend/src/datasources/open_meteo/places.rs (Place).
import type { ForecastProvider, InputView } from "./sources";

export interface Place {
	name: string;
	state: string | null;
	latitude: number;
	longitude: number;
}

export interface StationChoice {
	station_ref: number;
	network: string;
	name: string;
	state: string;
	distance_miles: number;
	elevation_difference_ft: number | null;
	first_year: number | null;
	last_year: number | null;
}

/** What a point would get, before it is saved. */
export interface Preview {
	latitude: number;
	longitude: number;
	timezone: string;
	elevation_ft: number | null;
	/** The plan, without a value in use: `in_use` and the rest are absent here. */
	inputs: Pick<InputView, "input" | "label" | "standing" | "sources">[];
	nearest_soil_station: StationChoice | null;
}

/** A point the owner chose in the location step, with what it would get. */
export interface ChosenPoint {
	latitude: number;
	longitude: number;
	place_name: string | null;
	preview: Preview;
}

/** Pinned stations by input name. */
export type Pins = Record<string, number>;

export interface ApplyRequest {
	location?: { latitude: number; longitude: number; place_name: string | null };
	pins?: { input: string; station_ref: number }[];
	forecast_provider?: { provider: ForecastProvider; key?: string };
}
