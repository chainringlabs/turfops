// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getSensors, mapSensor, unmapSensor } from "../../../api/client";
import type { SensorView } from "../../../types/sensors";
import MappedSensors from "./MappedSensors";
import PostedSensorForm from "./PostedSensorForm";

vi.mock("../../../api/client", () => ({
	getSensors: vi.fn(),
	mapSensor: vi.fn(),
	unmapSensor: vi.fn(),
}));
const sensors = vi.mocked(getSensors);
const map = vi.mocked(mapSensor);
const unmap = vi.mocked(unmapSensor);

const MAPPED: SensorView[] = [
	{
		id: 1,
		name: "Patio temperature",
		way_in: "HomeAssistant",
		entity_id: "sensor.patio_temperature",
		input: "air_temp",
		label: "Air temperature",
		rain_kind: null,
	},
	{
		id: 2,
		name: "Gauge",
		way_in: "Posted",
		entity_id: null,
		input: "rain",
		label: "Rain",
		rain_kind: "Counter",
	},
];

beforeEach(() => {
	sensors.mockReset();
	map.mockReset();
	unmap.mockReset();
	sensors.mockResolvedValue(MAPPED);
	map.mockResolvedValue(MAPPED);
	unmap.mockResolvedValue([MAPPED[1]]);
});

// @spec SETUP-MAP-010, SETUP-MAP-014
describe("a sensor that is not in Home Assistant", () => {
	it("is mapped by name and input, then shows the request that sends it a reading", async () => {
		const user = userEvent.setup();
		const onMapped = vi.fn();
		render(<PostedSensorForm enabled onMapped={onMapped} />);
		await user.type(screen.getByLabelText("Sensor name"), "Garden probe");
		await user.selectOptions(screen.getByLabelText("Measures"), "soil_temp_10");
		await user.click(screen.getByRole("button", { name: "Add sensor" }));
		expect(map).toHaveBeenCalledWith({
			name: "Garden probe",
			input: "soil_temp_10",
		});
		expect(onMapped).toHaveBeenCalled();
		const request = await screen.findByText(/curl -X POST/);
		expect(request.textContent).toContain(
			`${window.location.origin}/api/v1/sensor-readings`,
		);
		expect(request.textContent).toContain("Bearer $SENSOR_TOKEN");
		expect(request.textContent).toContain('"sensor": "Garden probe"');
	});

	it("names rain's kind", async () => {
		const user = userEvent.setup();
		render(<PostedSensorForm enabled onMapped={vi.fn()} />);
		await user.type(screen.getByLabelText("Sensor name"), "Gauge");
		await user.selectOptions(screen.getByLabelText("Measures"), "rain:Amount");
		await user.click(screen.getByRole("button", { name: "Add sensor" }));
		expect(map).toHaveBeenCalledWith({
			name: "Gauge",
			input: "rain",
			rain_kind: "Amount",
		});
	});

	it("says posted readings need SENSOR_TOKEN while it is not set", () => {
		render(<PostedSensorForm enabled={false} onMapped={vi.fn()} />);
		expect(
			screen.getByText(/needs SENSOR_TOKEN set on the server/),
		).toBeInTheDocument();
	});
});

// @spec SETUP-MAP-013
describe("the mapped sensors", () => {
	it("are listed with their input and way in, and unmapped at once", async () => {
		const user = userEvent.setup();
		const onUnmapped = vi.fn();
		render(<MappedSensors version={0} onUnmapped={onUnmapped} />);
		const rows = await screen.findAllByRole("listitem");
		expect(rows[0]).toHaveTextContent("Patio temperature");
		expect(rows[0]).toHaveTextContent("Air temperature");
		expect(rows[0]).toHaveTextContent("Home Assistant");
		expect(rows[1]).toHaveTextContent("Rain, as a counter");
		expect(rows[1]).toHaveTextContent("Posted");
		await user.click(within(rows[0]).getByRole("button", { name: "Unmap" }));
		expect(unmap).toHaveBeenCalledWith(1);
		expect(onUnmapped).toHaveBeenCalled();
	});
});
