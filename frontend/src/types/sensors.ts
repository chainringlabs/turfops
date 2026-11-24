// Mirrors backend/src/api/sources/home_assistant.rs, sensors.rs and sensor_check.rs.
import type { InUse } from "./sources";

export interface ConnectionView {
	connected: boolean;
	address: string | null;
}

export interface TestResult {
	answered: { location_name: string; version: string };
	numeric_sensors: number;
}

export type Suggestion =
	| "Humidity"
	| "SoilMoisture"
	| "RainCounter"
	| "RainAmount";

export interface EntityView {
	entity_id: string;
	friendly_name: string;
	value: number;
	unit: string | null;
	device_class: string | null;
	reported_at: string;
	suggestion: Suggestion | null;
	/** The input it is already mapped to, as named in requests. */
	mapped_to: string | null;
}

export type RainKind = "Counter" | "Amount";

export interface SensorView {
	id: number;
	name: string;
	way_in: "HomeAssistant" | "Posted";
	entity_id: string | null;
	input: string;
	label: string;
	rain_kind: RainKind | null;
}

export interface MappingRequest {
	name: string;
	entity_id?: string;
	input: string;
	rain_kind?: RainKind;
	replace?: boolean;
}

export interface Checked {
	value: number | null;
	unit: string | null;
	reported_at: string | null;
	otherwise: InUse | null;
	blocking: string[];
	warnings: string[];
}
