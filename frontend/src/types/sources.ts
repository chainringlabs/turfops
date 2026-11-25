// Mirrors backend/src/api/sources/read.rs and view.rs, backend/src/models/weather_basis.rs,
// backend/src/models/weather_record.rs (SourceKind), backend/src/models/forecast.rs
// (ForecastProvider) and backend/src/logic/soil_estimate/scores.rs.

export type Standing = "Measured" | "Estimated" | "NoSource";

export type SourceKind =
	| "Sensor"
	| "Station"
	| "Grid"
	| "Analysis"
	| "Estimate";

export type BasisKind = "Sensor" | "Station" | "Estimate";

export type ForecastProvider =
	| "OpenMeteo"
	| "NationalWeatherService"
	| "OpenWeatherMap";

export type Direction = "Rising" | "Falling";

export type Calibration = "NearestFive" | "StationAlone";

/** What a value rests on. */
export interface Basis {
	kind: BasisKind;
	source: string;
	distance_miles: number | null;
	approximate: boolean;
}

export interface PlannedSource {
	name: string;
	kind: SourceKind;
	distance_miles: number | null;
	pinned: boolean;
}

export interface InUse {
	value: number;
	observed_at: string;
	basis: Basis;
}

export interface CrossingAccuracy {
	threshold_f: number;
	direction: Direction;
	median_days: number;
	p80_days: number;
	datable: boolean;
}

export interface EstimateAccuracy {
	typical_error_f: number;
	fitted_on: Calibration;
	crossings: CrossingAccuracy[];
}

export interface InputView {
	/** The input's name in requests, such as `soil_temp_10`. */
	input: string;
	label: string;
	standing: Standing;
	sources: PlannedSource[];
	in_use: InUse | null;
	estimate_accuracy: EstimateAccuracy | null;
	pinned_station_ref: number | null;
}

export interface SourceView {
	id: number;
	name: string;
	kind: SourceKind;
	inputs: string[];
	last_success: string | null;
	last_failure: string | null;
	/** Set only while the source is failing. */
	last_error: string | null;
	held_from: string | null;
	held_through: string | null;
	wanted_from: string | null;
	attribution: string;
}

export interface Connections {
	home_assistant: boolean;
	openweathermap_key: boolean;
	posted_readings: boolean;
}

export interface LocationView {
	place_name: string | null;
	latitude: number;
	longitude: number;
	timezone: string;
}

/** How fetching the forecast is going (SYNC-CONN-007). */
export interface ForecastStatus {
	/** The chosen provider. */
	provider: ForecastProvider;
	/** When the forecast held was fetched; null when none is held. */
	fetched_at: string | null;
	/** Why the latest fetch failed or could not be made; null otherwise. */
	last_error: string | null;
}

export interface SourcesView {
	location: LocationView | null;
	inputs: InputView[];
	sources: SourceView[];
	forecast_provider: ForecastProvider;
	forecast: ForecastStatus;
	connections: Connections;
}

export const FORECAST_PROVIDER_NAMES: Record<ForecastProvider, string> = {
	OpenMeteo: "Open-Meteo",
	NationalWeatherService: "National Weather Service",
	OpenWeatherMap: "OpenWeatherMap",
};

export const SOURCE_KIND_LABELS: Record<SourceKind, string> = {
	Sensor: "Your sensor",
	Station: "Station",
	Grid: "Grid",
	Analysis: "Hourly analysis",
	Estimate: "Estimate",
};

export const STANDING_LABELS: Record<Standing, string> = {
	Measured: "Measured",
	Estimated: "Estimated",
	NoSource: "No source",
};

export const BASIS_KIND_LABELS: Record<BasisKind, string> = {
	Sensor: "Your sensor",
	Station: "Measured",
	Estimate: "Estimated for your location",
};
